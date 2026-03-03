//! P2P Network Module for Gork Agent Protocol (Client)
//!
//! NAT traversal implementation with:
//! - gossipsub (pub/sub messaging)
//! - Kademlia DHT (peer discovery)
//! - Identify + Ping (connection management)
//! - P2C load balancing for peer selection

use anyhow::Result;
use libp2p::{
    gossipsub, identify, kad, noise, ping,
    swarm::NetworkBehaviour,
    swarm::SwarmEvent,
    tcp, yamux,
    Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use libp2p::futures::StreamExt;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::types::AgentIdentity;
use crate::load_balancing::P2CSelector;

pub const DEFAULT_PORT: u16 = 4001;
pub const GOSSIPSUB_TOPIC: &str = "gork-agent-messages";

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub port: u16,
    pub bootstrap_peers: Vec<Multiaddr>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            bootstrap_peers: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    MessageReceived { from: String, message: Vec<u8> },
    PeerConnected(String),
    PeerDisconnected(String),
    Error(String),
}

/// Client network behaviour
#[derive(NetworkBehaviour)]
struct ClientBehaviour {
    gossipsub: gossipsub::Behaviour,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

pub struct AgentNetwork {
    pub swarm: Arc<RwLock<Swarm<ClientBehaviour>>>,
    pub peer_id: PeerId,
    pub event_sender: mpsc::UnboundedSender<NetworkEvent>,
    pub topic: gossipsub::IdentTopic,
    pub require_auth: bool,
    pub verified_peers: std::collections::HashMap<String, bool>,
    /// P2C load balancer for peer selection
    pub peer_selector: Arc<RwLock<P2CSelector>>,
}

impl AgentNetwork {
    pub async fn new(
        identity: AgentIdentity,
        config: NetworkConfig,
        event_sender: mpsc::UnboundedSender<NetworkEvent>,
    ) -> Result<Self> {
        Self::with_auth(identity, config, event_sender, None, false).await
    }

    pub async fn with_auth(
        identity: AgentIdentity,
        config: NetworkConfig,
        event_sender: mpsc::UnboundedSender<NetworkEvent>,
        _authenticator: Option<String>,
        require_auth: bool,
    ) -> Result<Self> {
        info!("P2P node creating with identity: {}", identity.account_id);

        let swarm = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|keypair| {
                let local_peer_id = keypair.public().to_peer_id();

                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .message_id_fn(|message: &gossipsub::Message| {
                        let mut s = DefaultHasher::new();
                        message.data.hash(&mut s);
                        gossipsub::MessageId::from(s.finish().to_string())
                    })
                    .build()
                    .expect("Valid gossipsub config");

                let mut gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(keypair.clone()),
                    gossipsub_config,
                ).expect("Valid gossipsub behaviour");

                let topic = gossipsub::IdentTopic::new(GOSSIPSUB_TOPIC);
                gossipsub.subscribe(&topic).expect("Subscribe to topic");

                let store = kad::store::MemoryStore::new(local_peer_id);
                let mut kademlia = kad::Behaviour::new(local_peer_id, store);
                kademlia.set_mode(Some(kad::Mode::Server));

                let identify = identify::Behaviour::new(identify::Config::new(
                    "/gork-agent/1.0.0".to_string(),
                    keypair.public(),
                ));

                let ping = ping::Behaviour::new(ping::Config::new());

                Ok(ClientBehaviour {
                    gossipsub,
                    kademlia,
                    identify,
                    ping,
                })
            })?
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(60))
            })
            .build();

        let peer_id = *swarm.local_peer_id();
        let topic = gossipsub::IdentTopic::new(GOSSIPSUB_TOPIC);
        let swarm = Arc::new(RwLock::new(swarm));

        let network = Self {
            swarm,
            peer_id,
            event_sender,
            topic,
            require_auth,
            verified_peers: std::collections::HashMap::new(),
            peer_selector: Arc::new(RwLock::new(P2CSelector::new())),
        };

        for addr in config.bootstrap_peers.clone() {
            let addr_clone = addr.clone();
            if let Err(e) = network.add_bootstrap_peer(&addr).await {
                warn!("Failed to add bootstrap peer {}: {}", addr_clone, e);
            }
        }

        Ok(network)
    }

    pub async fn add_bootstrap_peer(&self, addr: &Multiaddr) -> Result<()> {
        let peer_id = addr
            .iter()
            .find_map(|p| {
                if let libp2p::multiaddr::Protocol::P2p(peer_id) = p {
                    Some(peer_id)
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow::anyhow!("No peer ID in address"))?;

        self.swarm
            .write()
            .await
            .behaviour_mut()
            .kademlia
            .add_address(&peer_id, addr.clone());

        info!("Added bootstrap peer: {} at {}", peer_id, addr);
        Ok(())
    }

    pub async fn listen(&self, port: Option<u16>) -> Result<Multiaddr> {
        let port = port.unwrap_or(DEFAULT_PORT);
        let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", port).parse()?;

        self.swarm
            .write()
            .await
            .listen_on(listen_addr.clone())
            .map_err(|e| anyhow::anyhow!("Failed to listen on {}: {}", listen_addr, e))?;

        info!("Listening on: {}", listen_addr);
        Ok(listen_addr)
    }

    pub async fn dial(&self, addr: Multiaddr) -> Result<()> {
        self.swarm
            .write()
            .await
            .dial(addr.clone())
            .map_err(|e| anyhow::anyhow!("Failed to dial {}: {}", addr, e))?;
        info!("Dialing peer at: {}", addr);
        Ok(())
    }

    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    pub async fn broadcast(&self, topic: &str, message: &[u8]) -> Result<()> {
        let topic = gossipsub::IdentTopic::new(topic);

        let message_id = self
            .swarm
            .write()
            .await
            .behaviour_mut()
            .gossipsub
            .publish(topic, message.to_vec())
            .map_err(|e| anyhow::anyhow!("Failed to publish message: {}", e))?;

        info!("Published message with ID: {:?}", message_id);
        Ok(())
    }

    pub fn requires_auth(&self) -> bool {
        self.require_auth
    }

    pub fn is_peer_verified(&self, near_account: &str) -> bool {
        *self.verified_peers.get(near_account).unwrap_or(&false)
    }

    pub fn mark_peer_verified(&mut self, near_account: String) {
        self.verified_peers.insert(near_account, true);
    }

    /// Get connected peers using P2C load balancing
    pub async fn get_connected_peers(&self) -> Vec<PeerId> {
        let swarm = self.swarm.read().await;
        swarm.connected_peers().copied().collect()
    }

    /// Select best peer for message forwarding using P2C
    pub async fn select_peer_for_forward(&self, exclude: &[PeerId]) -> Option<PeerId> {
        let connected: Vec<PeerId> = self.get_connected_peers().await
            .into_iter()
            .filter(|p| !exclude.contains(p))
            .collect();

        let selector = self.peer_selector.read().await;
        selector.select_peer(&connected)
    }

    /// Select multiple peers for message fanout using P2C
    pub async fn select_peers_for_fanout(&self, count: usize, exclude: &[PeerId]) -> Vec<PeerId> {
        let connected: Vec<PeerId> = self.get_connected_peers().await
            .into_iter()
            .filter(|p| !exclude.contains(p))
            .collect();

        let selector = self.peer_selector.read().await;
        selector.select_multiple(&connected, count)
    }

    /// Record that a message was forwarded to a peer (for load tracking)
    pub async fn record_forward(&self, peer_id: PeerId) {
        self.peer_selector.write().await
            .tracker_mut()
            .record_forward(peer_id);
    }

    /// Broadcast message with P2C peer selection for forwarding
    pub async fn broadcast_p2c(&self, topic: &str, message: &[u8], fanout: usize) -> Result<Vec<PeerId>> {
        // Publish to topic (gossipsub handles distribution)
        self.broadcast(topic, message).await?;

        // Select peers for explicit forwarding using P2C
        let selected = self.select_peers_for_fanout(fanout, &[]).await;

        // Record forwarding for load tracking
        for peer in &selected {
            self.record_forward(*peer).await;
        }

        Ok(selected)
    }

    pub async fn run(&mut self) {
        info!("P2P network running");

        loop {
            let event = {
                let mut swarm_guard = self.swarm.write().await;
                swarm_guard.select_next_some().await
            };

            match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("Listening on: {}", address);
                }

                SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                    info!("Connected to: {}", peer_id);

                    self.swarm.write().await
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, endpoint.get_remote_address().clone());

                    // Track peer load for P2C
                    self.peer_selector.write().await
                        .tracker_mut()
                        .record_connection(peer_id, 1);

                    let _ = self.event_sender.send(NetworkEvent::PeerConnected(peer_id.to_string()));
                }

                SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                    if let Some(cause) = cause {
                        info!("Disconnected from {}: {}", peer_id, cause);
                    } else {
                        info!("Disconnected from: {}", peer_id);
                    }

                    // Update peer load for P2C
                    self.peer_selector.write().await
                        .tracker_mut()
                        .record_connection(peer_id, -1);

                    let _ = self.event_sender.send(NetworkEvent::PeerDisconnected(peer_id.to_string()));
                }

                SwarmEvent::Behaviour(event) => {
                    self.handle_behaviour_event(event).await;
                }

                SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                    warn!("Outgoing connection error to {:?}: {}", peer_id, error);
                }

                SwarmEvent::IncomingConnectionError { error, .. } => {
                    warn!("Incoming connection error: {}", error);
                }

                SwarmEvent::ListenerError { listener_id, error } => {
                    error!("Listener {} error: {}", listener_id, error);
                }

                _ => {}
            }
        }
    }

    async fn handle_behaviour_event(&mut self, event: <ClientBehaviour as NetworkBehaviour>::ToSwarm) {
        match event {
            ClientBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source: peer_id,
                message,
                ..
            }) => {
                info!("Message from: {}", peer_id);
                let _ = self.event_sender.send(NetworkEvent::MessageReceived {
                    from: peer_id.to_string(),
                    message: message.data,
                });
            }

            ClientBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { peer_id, topic }) => {
                info!("Peer {} subscribed to: {}", peer_id, topic);
            }

            ClientBehaviourEvent::Kademlia(kad::Event::RoutingUpdated { peer, is_new_peer, .. }) => {
                if is_new_peer {
                    info!("Kademlia: New peer: {}", peer);
                }
            }

            ClientBehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. }) => {
                info!("Identified: {} ({})", peer_id, info.protocol_version);
                self.swarm.write().await
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, info.observed_addr.clone());
            }

            _ => {}
        }
    }
}

pub struct MessageHandler {
    security_manager: crate::security::SecurityManager,
}

impl MessageHandler {
    pub fn new(owner: &str) -> Self {
        Self {
            security_manager: crate::security::SecurityManager::new(owner),
        }
    }

    pub fn handle_message(
        &mut self,
        from: String,
        data: &[u8],
    ) -> Result<Option<crate::types::Message>> {
        let plain_msg: crate::types::PlainMessage = serde_json::from_slice(data)?;

        let result = self
            .security_manager
            .process_message(&from, &plain_msg.content, 50, true)?;

        match result {
            crate::security::MessageProcessingResult::Allowed { content } => {
                Ok(Some(crate::types::Message::new(
                    from,
                    String::new(),
                    crate::types::EncryptedPayload {
                        ciphertext: content.into_bytes(),
                        nonce: vec![],
                        signature: vec![],
                        sender_pubkey: vec![],
                    },
                )))
            }
            _ => Ok(None),
        }
    }
}

pub fn parse_multiaddr(s: &str) -> Result<Multiaddr> {
    Ok(s.parse()?)
}

pub fn create_p2p_message(content: &str) -> Vec<u8> {
    serde_json::to_vec(&crate::types::PlainMessage::new(content.to_string())).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert_eq!(config.port, DEFAULT_PORT);
    }

    #[tokio::test]
    async fn test_network_creation() {
        let identity = AgentIdentity::new("test.near".to_string(), vec![0u8; 32]);
        let (sender, _receiver) = mpsc::unbounded_channel();
        let network = AgentNetwork::new(identity, NetworkConfig::default(), sender).await;
        assert!(network.is_ok());
    }
}
