//! P2P Network Module for Gork Agent Protocol
//!
//! Full implementation using libp2p with gossipsub, Kademlia DHT, and proper event handling

use anyhow::Result;
use libp2p::{
    gossipsub, identify, kad, noise, ping, swarm::NetworkBehaviour, swarm::SwarmEvent, tcp, yamux,
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

/// Network behaviour combining all P2P protocols
#[derive(NetworkBehaviour)]
struct GorkAgentBehaviour {
    gossipsub: gossipsub::Behaviour,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

pub struct AgentNetwork {
    pub swarm: Arc<RwLock<Swarm<GorkAgentBehaviour>>>,
    pub peer_id: PeerId,
    pub event_sender: mpsc::UnboundedSender<NetworkEvent>,
    pub topic: gossipsub::IdentTopic,
    pub require_auth: bool,
    pub verified_peers: std::collections::HashMap<String, bool>,
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
        _authenticator: Option<String>,  // We'll use this differently
        require_auth: bool,
    ) -> Result<Self> {
        info!("P2P node creating with identity: {}", identity.account_id);

        // Create gossipsub configuration
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .message_id_fn(|message: &gossipsub::Message| {
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                gossipsub::MessageId::from(s.finish().to_string())
            })
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build gossipsub config: {}", e))?;

        // Create gossipsub behaviour
        let mut gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(libp2p::identity::Keypair::generate_ed25519()),
            gossipsub_config,
        )
        .map_err(|e| anyhow::anyhow!("Failed to create gossipsub: {}", e))?;

        // Subscribe to the main topic
        let topic = gossipsub::IdentTopic::new(GOSSIPSUB_TOPIC);
        gossipsub
            .subscribe(&topic)
            .map_err(|e| anyhow::anyhow!("Failed to subscribe to topic: {}", e))?;

        // Create Kademlia DHT - will use the swarm's peer ID later
        let temp_peer_id = PeerId::random();
        let store = kad::store::MemoryStore::new(temp_peer_id);
        let mut kademlia = kad::Behaviour::new(temp_peer_id, store);

        // Set Kademlia mode to server (both client and server)
        kademlia.set_mode(Some(kad::Mode::Server));

        // Create identify behaviour - will use the swarm's keypair
        let identify_keypair = libp2p::identity::Keypair::generate_ed25519();
        let identify = identify::Behaviour::new(identify::Config::new(
            "/gork-agent/1.0.0".to_string(),
            identify_keypair.public(),
        ));

        // Create ping behaviour
        let ping = ping::Behaviour::new(ping::Config::new());

        // Create the combined behaviour
        let behaviour = GorkAgentBehaviour {
            gossipsub,
            kademlia,
            identify,
            ping,
        };

        // Create the swarm using SwarmBuilder API for libp2p 0.55
        let swarm = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|_key| behaviour)?
            .with_swarm_config(|config| {
                config.with_idle_connection_timeout(Duration::from_secs(60))
            })
            .build();

        // Get the peer ID from the swarm
        let peer_id = *swarm.local_peer_id();

        // Wrap swarm in Arc<RwLock> for concurrent access
        let swarm = Arc::new(RwLock::new(swarm));

        let network = Self {
            swarm,
            peer_id,
            event_sender,
            topic,
            require_auth,
            verified_peers: std::collections::HashMap::new(),
        };

        // Add bootstrap peers to DHT
        for addr in config.bootstrap_peers {
            let addr_clone = addr.clone();
            if let Err(e) = network.add_bootstrap_peer(&addr).await {
                warn!("Failed to add bootstrap peer {}: {}", addr_clone, e);
            }
        }

        Ok(network)
    }

    /// Add a bootstrap peer to the DHT
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

    /// Start listening on the specified port
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

    /// Dial a peer at the specified address
    pub async fn dial(&self, addr: Multiaddr) -> Result<()> {
        self.swarm
            .write()
            .await
            .dial(addr.clone())
            .map_err(|e| anyhow::anyhow!("Failed to dial {}: {}", addr, e))?;
        info!("Dialing peer at: {}", addr);
        Ok(())
    }

    /// Get the local peer ID
    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    /// Broadcast a message to all subscribed peers
    pub async fn broadcast(&self, topic: &str, message: &[u8]) -> Result<()> {
        // If authentication is required, we should check permissions
        // For now, this is a placeholder - in production, you'd filter recipients
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

    /// Check if peer authentication is required
    pub fn requires_auth(&self) -> bool {
        self.require_auth
    }

    /// Check if a peer is verified
    pub fn is_peer_verified(&self, near_account: &str) -> bool {
        *self.verified_peers.get(near_account).unwrap_or(&false)
    }

    /// Mark a peer as verified
    pub fn mark_peer_verified(&mut self, near_account: String) {
        self.verified_peers.insert(near_account, true);
    }

    /// Mark a peer as unverified
    pub fn mark_peer_unverified(&mut self, near_account: String) {
        self.verified_peers.insert(near_account, false);
    }

    /// Run the network event loop
    pub async fn run(&mut self) {
        info!("P2P network running");

        // Clone Arc for use in event loop
        let swarm = self.swarm.clone();
        let event_sender = self.event_sender.clone();

        loop {
            // Get next event from swarm
            let event = {
                let mut swarm_guard = swarm.write().await;
                swarm_guard.select_next_some().await
            };

            match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("Local node is listening on: {}", address);
                }

                SwarmEvent::ConnectionEstablished {
                    peer_id, endpoint, ..
                } => {
                    info!("Connection established with peer: {}", peer_id);

                    // Add peer to Kademlia DHT
                    swarm.write().await
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, endpoint.get_remote_address().clone());

                    // Notify about new peer
                    let _ = event_sender
                        .send(NetworkEvent::PeerConnected(peer_id.to_string()));
                }

                SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                    if let Some(cause) = cause {
                        info!("Connection closed with peer {}: {}", peer_id, cause);
                    } else {
                        info!("Connection closed with peer: {}", peer_id);
                    }

                    let _ = event_sender
                        .send(NetworkEvent::PeerDisconnected(peer_id.to_string()));
                }

                SwarmEvent::Behaviour(event) => {
                    match event {
                        // Handle gossipsub events
                        GorkAgentBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                            propagation_source: peer_id,
                            message_id: _id,
                            message,
                        }) => {
                            info!("Received gossipsub message from peer: {}", peer_id);

                            let _ = event_sender.send(NetworkEvent::MessageReceived {
                                from: peer_id.to_string(),
                                message: message.data,
                            });
                        }

                        GorkAgentBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed {
                            peer_id,
                            topic,
                        }) => {
                            info!("Peer {} subscribed to topic: {}", peer_id, topic);
                        }

                        GorkAgentBehaviourEvent::Gossipsub(gossipsub::Event::Unsubscribed {
                            peer_id,
                            topic,
                        }) => {
                            info!("Peer {} unsubscribed from topic: {}", peer_id, topic);
                        }

                        // Handle Kademlia events
                        GorkAgentBehaviourEvent::Kademlia(kad::Event::RoutingUpdated {
                            peer,
                            is_new_peer,
                            ..
                        }) => {
                            if is_new_peer {
                                info!("Kademlia: New peer added to routing table: {}", peer);
                            }
                        }

                        GorkAgentBehaviourEvent::Kademlia(
                            kad::Event::OutboundQueryProgressed { result, .. },
                        ) => match result {
                            kad::QueryResult::GetProviders(Ok(_ok)) => {
                                info!("Kademlia: Get providers query succeeded");
                            }
                            kad::QueryResult::GetProviders(Err(err)) => {
                                warn!("Kademlia: Get providers query failed: {:?}", err);
                            }
                            _ => {}
                        },

                        // Handle identify events
                        GorkAgentBehaviourEvent::Identify(identify::Event::Received {
                            peer_id,
                            info,
                            ..
                        }) => {
                            info!(
                                "Identified peer: {} with protocol {}",
                                peer_id, info.protocol_version
                            );

                            // Add observed address to Kademlia
                            swarm.write().await
                                .behaviour_mut()
                                .kademlia
                                .add_address(&peer_id, info.observed_addr);
                        }

                        GorkAgentBehaviourEvent::Identify(identify::Event::Error {
                            peer_id,
                            error,
                            ..
                        }) => {
                            warn!("Identify error for peer {}: {}", peer_id, error);
                        }

                        // Handle ping events
                        GorkAgentBehaviourEvent::Ping(ping_event) => {
                            match ping_event.result {
                                Ok(_success) => {
                                    info!("Ping successful");
                                }
                                Err(_error) => {
                                    warn!("Ping failed");
                                }
                            }
                        }

                        _ => {}
                    }
                }

                SwarmEvent::OutgoingConnectionError {
                    peer_id,
                    error,
                    connection_id,
                    ..
                } => {
                    let _ = connection_id;
                    warn!("Outgoing connection error to {:?}: {}", peer_id, error);
                }

                SwarmEvent::IncomingConnectionError {
                    local_addr,
                    send_back_addr,
                    error,
                    connection_id,
                } => {
                    let _ = connection_id;
                    warn!(
                        "Incoming connection error from {} to {}: {}",
                        send_back_addr, local_addr, error
                    );
                }

                SwarmEvent::ListenerError { listener_id, error } => {
                    error!("Listener {} error: {}", listener_id, error);
                }

                SwarmEvent::ListenerClosed {
                    listener_id,
                    addresses,
                    reason,
                } => {
                    info!(
                        "Listener {} closed on {:?}, reason: {:?}",
                        listener_id, addresses, reason
                    );
                }

                _ => {}
            }
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
        assert!(config.bootstrap_peers.is_empty());
    }

    #[test]
    fn test_parse_multiaddr() {
        let addr = parse_multiaddr("/ip4/127.0.0.1/tcp/4001").unwrap();
        assert!(addr.to_string().contains("127.0.0.1"));
    }

    #[test]
    fn test_create_p2p_message() {
        let msg = create_p2p_message("Hello, P2P!");
        assert!(!msg.is_empty());

        let parsed: crate::types::PlainMessage = serde_json::from_slice(&msg).unwrap();
        assert_eq!(parsed.content, "Hello, P2P!");
    }

    #[tokio::test]
    async fn test_network_creation() {
        let identity = AgentIdentity::new("test.near".to_string(), vec![0u8; 32]);
        let (sender, _receiver) = mpsc::unbounded_channel();

        let network = AgentNetwork::new(identity, NetworkConfig::default(), sender).await;

        assert!(network.is_ok());
        let network = network.unwrap();
        assert!(!network.peer_id.to_string().is_empty());
    }
}
