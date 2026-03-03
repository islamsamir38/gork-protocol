//! Hybrid Relay Server for Gork Agent Protocol
//!
//! Three roles:
//! 1. Bootstrap Node - Peer discovery via Kademlia DHT
//! 2. Circuit Relay - Fallback for peers behind NAT
//!
//! Uses P2C (Power of Two Random Choices) for load balancing

use anyhow::Result;
use libp2p::{
    gossipsub, identify, kad, noise, ping, relay,
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

use crate::load_balancing::{RelaySelector, RelayInfo};

pub const DEFAULT_PORT: u16 = 4001;
pub const GOSSIPSUB_TOPIC: &str = "gork-agent-messages";

#[derive(Debug, Clone)]
pub struct RelayConfig {
    pub port: u16,
    pub max_circuits: usize,
    pub max_circuit_duration_secs: u64,
    pub max_circuit_bytes: u64,
    pub enable_metrics: bool,
    pub metrics_port: u16,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            max_circuits: 1000,
            max_circuit_duration_secs: 120,
            max_circuit_bytes: 1024 * 1024,
            enable_metrics: false,
            metrics_port: 9090,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RelayEvent {
    PeerConnected(String),
    PeerDisconnected(String),
    CircuitEstablished { src: String, dst: String },
    CircuitClosed { src: String, dst: String },
    Error(String),
}

#[derive(NetworkBehaviour)]
struct RelayServerBehaviour {
    gossipsub: gossipsub::Behaviour,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
    relay: relay::Behaviour,
}

pub struct RelayServer {
    pub swarm: Arc<RwLock<Swarm<RelayServerBehaviour>>>,
    pub peer_id: PeerId,
    pub config: RelayConfig,
    pub event_sender: mpsc::UnboundedSender<RelayEvent>,
    pub topic: gossipsub::IdentTopic,
    /// P2C selector for choosing among multiple relays (client-side)
    pub relay_selector: Arc<RwLock<RelaySelector>>,
}

impl RelayServer {
    pub async fn new(config: RelayConfig) -> Result<Self> {
        let (event_sender, _) = mpsc::unbounded_channel();
        Self::with_events(config, event_sender).await
    }

    pub async fn with_events(
        config: RelayConfig,
        event_sender: mpsc::UnboundedSender<RelayEvent>,
    ) -> Result<Self> {
        info!("🌐 Creating hybrid relay server on port {}", config.port);

        let swarm = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|keypair| {
                let local_peer_id = keypair.public().to_peer_id();

                // Gossipsub
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

                // Kademlia
                let store = kad::store::MemoryStore::new(local_peer_id);
                let mut kademlia = kad::Behaviour::new(local_peer_id, store);
                kademlia.set_mode(Some(kad::Mode::Server));

                // Identify
                let identify = identify::Behaviour::new(identify::Config::new(
                    "/gork-relay/1.0.0".to_string(),
                    keypair.public(),
                ));

                // Ping
                let ping = ping::Behaviour::new(ping::Config::new());

                // Circuit relay (server mode)
                let relay_config = relay::Config {
                    max_reservations: config.max_circuits,
                    max_reservations_per_peer: 10,
                    reservation_duration: Duration::from_secs(3600),
                    reservation_rate_limiters: vec![],
                    max_circuits: config.max_circuits,
                    max_circuits_per_peer: 10,
                    max_circuit_duration: Duration::from_secs(config.max_circuit_duration_secs),
                    max_circuit_bytes: config.max_circuit_bytes,
                    circuit_src_rate_limiters: vec![],
                };
                let relay = relay::Behaviour::new(local_peer_id, relay_config);

                Ok(RelayServerBehaviour {
                    gossipsub,
                    kademlia,
                    identify,
                    ping,
                    relay,
                })
            })?
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(60))
            })
            .build();

        let peer_id = *swarm.local_peer_id();
        let topic = gossipsub::IdentTopic::new(GOSSIPSUB_TOPIC);
        let swarm = Arc::new(RwLock::new(swarm));

        info!("✅ Relay server created with Peer ID: {}", peer_id);

        Ok(Self {
            swarm,
            peer_id,
            config,
            event_sender,
            topic,
            relay_selector: Arc::new(RwLock::new(RelaySelector::new())),
        })
    }

    pub async fn listen(&self) -> Result<Multiaddr> {
        let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", self.config.port).parse()?;

        self.swarm
            .write()
            .await
            .listen_on(listen_addr.clone())
            .map_err(|e| anyhow::anyhow!("Failed to listen on {}: {}", listen_addr, e))?;

        info!("📡 Relay listening on: {}", listen_addr);
        Ok(listen_addr)
    }

    pub fn connection_string(&self, ip: &str) -> String {
        format!("/ip4/{}/tcp/{}/p2p/{}", ip, self.config.port, self.peer_id)
    }

    pub async fn run(&mut self) {
        info!("🚀 Relay server running");
        info!("   Roles: Bootstrap + Circuit Relay");
        info!("   Max circuits: {}", self.config.max_circuits);

        loop {
            let event = {
                let mut swarm_guard = self.swarm.write().await;
                swarm_guard.select_next_some().await
            };

            match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("📍 Listening on: {}", address);
                }

                SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                    info!("✅ Peer connected: {}", peer_id);
                    
                    self.swarm.write().await
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, endpoint.get_remote_address().clone());

                    let _ = self.event_sender.send(RelayEvent::PeerConnected(peer_id.to_string()));
                }

                SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                    if let Some(cause) = cause {
                        info!("❌ Peer disconnected: {} ({})", peer_id, cause);
                    } else {
                        info!("❌ Peer disconnected: {}", peer_id);
                    }
                    let _ = self.event_sender.send(RelayEvent::PeerDisconnected(peer_id.to_string()));
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

                _ => {}
            }
        }
    }

    async fn handle_behaviour_event(&self, event: <RelayServerBehaviour as NetworkBehaviour>::ToSwarm) {
        match event {
            RelayServerBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source: peer_id,
                message,
                ..
            }) => {
                info!("📨 Message from: {} ({} bytes)", peer_id, message.data.len());
            }

            RelayServerBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { peer_id, topic }) => {
                info!("📢 Peer {} subscribed to: {}", peer_id, topic);
            }

            RelayServerBehaviourEvent::Kademlia(kad::Event::RoutingUpdated { peer, is_new_peer, .. }) => {
                if is_new_peer {
                    info!("🔍 Kademlia: New peer: {}", peer);
                }
            }

            RelayServerBehaviourEvent::Relay(relay::Event::ReservationReqAccepted {
                src_peer_id,
                renewed,
                ..
            }) => {
                let status = if renewed { "renewed" } else { "new" };
                info!("🔌 Relay reservation {} from: {}", status, src_peer_id);
            }

            RelayServerBehaviourEvent::Relay(relay::Event::CircuitReqAccepted {
                src_peer_id,
                dst_peer_id,
                ..
            }) => {
                info!("🔀 Circuit established: {} → {}", src_peer_id, dst_peer_id);

                // Track circuit for load balancing
                if let Ok(mut selector) = self.relay_selector.try_write() {
                    if let Some(relay_info) = selector.relays().iter().find(|r| r.peer_id == self.peer_id).cloned() {
                        selector.add_relay(RelayInfo {
                            active_circuits: relay_info.active_circuits + 1,
                            ..relay_info
                        });
                    }
                }

                let _ = self.event_sender.send(RelayEvent::CircuitEstablished {
                    src: src_peer_id.to_string(),
                    dst: dst_peer_id.to_string(),
                });
            }

            RelayServerBehaviourEvent::Relay(relay::Event::CircuitClosed {
                src_peer_id,
                dst_peer_id,
                ..
            }) => {
                info!("🔌 Circuit closed: {} → {}", src_peer_id, dst_peer_id);

                // Track circuit for load balancing
                if let Ok(mut selector) = self.relay_selector.try_write() {
                    if let Some(relay_info) = selector.relays().iter().find(|r| r.peer_id == self.peer_id).cloned() {
                        selector.add_relay(RelayInfo {
                            active_circuits: relay_info.active_circuits.saturating_sub(1),
                            ..relay_info
                        });
                    }
                }

                let _ = self.event_sender.send(RelayEvent::CircuitClosed {
                    src: src_peer_id.to_string(),
                    dst: dst_peer_id.to_string(),
                });
            }

            RelayServerBehaviourEvent::Identify(identify::Event::Received {
                peer_id,
                info,
                ..
            }) => {
                info!("🆔 Identified: {} ({})", peer_id, info.protocol_version);
                self.swarm.write().await
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, info.observed_addr.clone());
            }

            RelayServerBehaviourEvent::Ping(ping::Event { peer, result, .. }) => {
                match result {
                    Ok(rtt) => {
                        info!("🏓 Ping to {}: {}ms", peer, rtt.as_millis());
                    }
                    Err(e) => {
                        warn!("🏓 Ping to {} failed: {}", peer, e);
                    }
                }
            }

            _ => {}
        }
    }

    pub async fn stats(&self) -> RelayStats {
        let swarm = self.swarm.read().await;
        let connected_peers = swarm.connected_peers().count();
        
        RelayStats {
            peer_id: self.peer_id.to_string(),
            port: self.config.port,
            connected_peers,
        }
    }

    /// Add a known relay to the selector (for clients)
    pub async fn add_known_relay(&self, peer_id: PeerId, addr: Multiaddr) {
        let mut selector = self.relay_selector.write().await;
        selector.add_relay(RelayInfo {
            peer_id,
            addr,
            active_circuits: 0,
            latency: None,
            last_success: None,
        });
    }

    /// Select best relay using P2C
    pub async fn select_best_relay(&self) -> Option<(PeerId, Multiaddr)> {
        let selector = self.relay_selector.read().await;
        selector.select_relay().map(|r| (r.peer_id, r.addr.clone()))
    }

    /// Get all known relays
    pub async fn known_relays(&self) -> Vec<RelayInfo> {
        self.relay_selector.read().await.relays().to_vec()
    }

    /// Update relay latency measurement
    pub async fn update_relay_latency(&self, peer_id: PeerId, latency: Duration) {
        let mut selector = self.relay_selector.write().await;
        if let Some(relay) = selector.relays().iter().find(|r| r.peer_id == peer_id).cloned() {
            selector.add_relay(RelayInfo {
                latency: Some(latency),
                last_success: Some(std::time::Instant::now()),
                ..relay
            });
        }
    }
}

#[derive(Debug, Clone)]
pub struct RelayStats {
    pub peer_id: String,
    pub port: u16,
    pub connected_peers: usize,
}

pub async fn start_metrics_server(port: u16, peer_id: String, mut stats_rx: mpsc::Receiver<RelayStats>) {
    use tokio::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => {
            info!("📊 Metrics server listening on http://0.0.0.0:{}", port);
            l
        }
        Err(e) => {
            error!("Failed to start metrics server: {}", e);
            return;
        }
    };

    let mut current_stats = RelayStats {
        peer_id: peer_id.clone(),
        port: 4001,
        connected_peers: 0,
    };

    loop {
        tokio::select! {
            Some(new_stats) = stats_rx.recv() => {
                current_stats = new_stats;
            }

            accept_result = listener.accept() => {
                match accept_result {
                    Ok((mut socket, _)) => {
                        let stats = current_stats.clone();
                        
                        tokio::spawn(async move {
                            let mut buffer = [0; 1024];
                            if let Ok(n) = socket.read(&mut buffer).await {
                                let request = String::from_utf8_lossy(&buffer[..n]);

                                let response = if request.contains("GET /metrics") {
                                    format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n\
                                        # HELP gork_relay_up Relay is running\n\
                                        # TYPE gork_relay_up gauge\n\
                                        gork_relay_up 1\n\
                                        # HELP gork_relay_connected_peers Connected peers\n\
                                        # TYPE gork_relay_connected_peers gauge\n\
                                        gork_relay_connected_peers {}\n\
                                        # EOF\n",
                                        stats.connected_peers
                                    )
                                } else if request.contains("GET /health") {
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n\
                                    {\"status\":\"healthy\",\"relay\":\"gork-relay\"}\n".to_string()
                                } else {
                                    "HTTP/1.1 404 Not Found\r\n\r\n".to_string()
                                };

                                let _ = socket.write_all(response.as_bytes()).await;
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Metrics server error: {}", e);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_relay_creation() {
        let config = RelayConfig::default();
        let relay = RelayServer::new(config).await;
        assert!(relay.is_ok());
    }
}
