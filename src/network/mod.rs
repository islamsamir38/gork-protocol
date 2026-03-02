//! P2P Network Module for Gork Agent Protocol
//! 
//! Simplified implementation using libp2p

use anyhow::Result;
use libp2p::{Multiaddr, PeerId, identity};
use tokio::sync::mpsc;
use tracing::info;

use crate::types::AgentIdentity;

pub const DEFAULT_PORT: u16 = 4001;

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub port: u16,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self { port: DEFAULT_PORT }
    }
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    MessageReceived { from: String, message: Vec<u8> },
    PeerConnected(String),
    PeerDisconnected(String),
    Error(String),
}

pub struct AgentNetwork {
    peer_id: PeerId,
    event_sender: mpsc::UnboundedSender<NetworkEvent>,
}

impl AgentNetwork {
    pub async fn new(
        _identity: AgentIdentity,
        _config: NetworkConfig,
        event_sender: mpsc::UnboundedSender<NetworkEvent>,
    ) -> Result<Self> {
        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());
        
        info!("P2P node created with peer ID: {}", peer_id);

        Ok(Self {
            peer_id,
            event_sender,
        })
    }

    pub fn listen(&mut self, port: Option<u16>) -> Result<Multiaddr> {
        let port = port.unwrap_or(DEFAULT_PORT);
        let addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", port).parse()?;
        info!("Would listen on: {}", addr);
        Ok(addr)
    }

    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    pub fn broadcast(&mut self, _topic: &str, _message: &[u8]) -> Result<()> {
        info!("Broadcast not yet implemented");
        Ok(())
    }

    pub async fn run(&mut self) {
        info!("P2P network running (stub mode)");
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
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

    pub fn handle_message(&mut self, from: String, data: &[u8]) -> Result<Option<crate::types::Message>> {
        let plain_msg: crate::types::PlainMessage = serde_json::from_slice(data)?;

        let result = self.security_manager.process_message(&from, &plain_msg.content, 50, true)?;

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
