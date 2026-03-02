//! Gork Agent Protocol
//! 
//! P2P agent-to-agent communication with NEAR integration

pub mod types;
pub mod crypto;
pub mod storage;
pub mod near;
pub mod registry;
pub mod security;
pub mod network;

use anyhow::Result;
use std::path::PathBuf;

use crate::crypto::MessageCrypto;
use crate::storage::AgentStorage;
use crate::types::{AgentConfig, AgentIdentity, Message, PlainMessage};

/// Default storage path
pub fn default_storage_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".gork-agent")
}

/// Main agent struct
pub struct Agent {
    pub config: AgentConfig,
    pub crypto: MessageCrypto,
    pub storage: AgentStorage,
}

impl Agent {
    /// Create or load agent
    pub fn init(account_id: String, network: &str) -> Result<Self> {
        let storage_path = default_storage_path();
        std::fs::create_dir_all(&storage_path)?;
        
        let storage = AgentStorage::open(&storage_path)?;
        
        // Try to load existing config
        if let Some(config) = storage.load_config()? {
            let crypto = MessageCrypto::new()?;
            return Ok(Self { config, crypto, storage });
        }

        // Create new identity
        let crypto = MessageCrypto::new()?;
        let public_key = crypto.public_key();
        
        let identity = AgentIdentity::new(account_id, public_key);
        let config = AgentConfig {
            identity,
            storage_path: storage_path.to_string_lossy().to_string(),
            network_id: network.to_string(),
        };

        storage.save_config(&config)?;
        storage.save_identity(&config.identity)?;

        Ok(Self { config, crypto, storage })
    }

    /// Load existing agent
    pub fn load() -> Result<Option<Self>> {
        let storage_path = default_storage_path();
        if !storage_path.exists() {
            return Ok(None);
        }

        let storage = AgentStorage::open(&storage_path)?;
        let config = storage.load_config()?;
        
        match config {
            Some(cfg) => {
                let crypto = MessageCrypto::new()?;
                Ok(Some(Self { config: cfg, crypto, storage }))
            }
            None => Ok(None),
        }
    }

    /// Get agent identity
    pub fn identity(&self) -> &AgentIdentity {
        &self.config.identity
    }

    /// Get account ID
    pub fn account_id(&self) -> &str {
        &self.config.identity.account_id
    }

    /// Get capabilities
    pub fn capabilities(&self) -> &[String] {
        &self.config.identity.capabilities
    }

    /// Add capability
    pub fn add_capability(&mut self, capability: String) -> Result<()> {
        if !self.config.identity.capabilities.contains(&capability) {
            self.config.identity.capabilities.push(capability);
            self.storage.save_identity(&self.config.identity)?;
        }
        Ok(())
    }

    /// Send message to another agent
    pub fn send(&self, to: &str, content: &str) -> Result<Message> {
        let plain = PlainMessage::new(content.to_string());
        let plaintext = plain.to_bytes();

        // For Phase 1, we store the message locally
        // In Phase 2, this will go through P2P network
        let payload = crate::types::EncryptedPayload {
            ciphertext: plaintext.clone(),
            nonce: vec![],
            signature: self.crypto.sign(&plaintext)?,
            sender_pubkey: self.crypto.public_key(),
        };

        let message = Message::new(
            self.config.identity.account_id.clone(),
            to.to_string(),
            payload,
        );

        // Store in our outbox (for now)
        // In production, this would be sent via P2P
        
        Ok(message)
    }

    /// Receive message from another agent
    pub fn receive(&mut self, message: Message) -> Result<()> {
        self.storage.save_message(&message)?;
        Ok(())
    }

    /// Get inbox
    pub fn inbox(&self) -> Result<Vec<Message>> {
        self.storage.get_messages()
    }

    /// Get messages from specific sender
    pub fn messages_from(&self, from: &str) -> Result<Vec<Message>> {
        self.storage.get_messages_from(from)
    }

    /// Clear inbox
    pub fn clear_inbox(&self) -> Result<()> {
        self.storage.clear_inbox()
    }

    /// Get agent status
    pub fn status(&self) -> AgentStatus {
        AgentStatus {
            account_id: self.config.identity.account_id.clone(),
            capabilities: self.config.identity.capabilities.clone(),
            network: self.config.network_id.clone(),
            message_count: self.storage.get_messages().map(|m| m.len()).unwrap_or(0),
        }
    }
}

/// Agent status info
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentStatus {
    pub account_id: String,
    pub capabilities: Vec<String>,
    pub network: String,
    pub message_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_creation() {
        // Use temp directory for test
        let temp_dir = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", temp_dir.path().to_string_lossy().to_string());
        
        let agent = Agent::init("test.near".to_string(), "testnet").unwrap();
        assert_eq!(agent.account_id(), "test.near");
    }
}
