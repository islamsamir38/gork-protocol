use anyhow::Result;
use rocksdb::{DB, Options};
use std::path::Path;

use crate::types::{AgentIdentity, Message, AgentConfig};

/// Storage keys
mod keys {
    pub const IDENTITY: &str = "identity";
    pub const CONFIG: &str = "config";
    pub const MESSAGE_PREFIX: &str = "msg:";
    pub const INBOX_PREFIX: &str = "inbox:";
}

/// Persistent storage for agent data
pub struct AgentStorage {
    db: DB,
}

impl AgentStorage {
    /// Open or create storage at path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_max_open_files(100);
        
        let db = DB::open(&opts, path)?;
        Ok(Self { db })
    }

    /// Save agent identity
    pub fn save_identity(&self, identity: &AgentIdentity) -> Result<()> {
        let data = serde_json::to_vec(identity)?;
        self.db.put(keys::IDENTITY, data)?;
        Ok(())
    }

    /// Load agent identity
    pub fn load_identity(&self) -> Result<Option<AgentIdentity>> {
        match self.db.get(keys::IDENTITY)? {
            Some(data) => Ok(Some(serde_json::from_slice(&data)?)),
            None => Ok(None),
        }
    }

    /// Save agent config
    pub fn save_config(&self, config: &AgentConfig) -> Result<()> {
        let data = serde_json::to_vec(config)?;
        self.db.put(keys::CONFIG, data)?;
        Ok(())
    }

    /// Load agent config
    pub fn load_config(&self) -> Result<Option<AgentConfig>> {
        match self.db.get(keys::CONFIG)? {
            Some(data) => Ok(Some(serde_json::from_slice(&data)?)),
            None => Ok(None),
        }
    }

    /// Save message to inbox
    pub fn save_message(&self, message: &Message) -> Result<()> {
        let key = format!("{}{}:{}", keys::INBOX_PREFIX, message.from, message.id);
        let data = serde_json::to_vec(message)?;
        self.db.put(key.as_bytes(), data)?;
        Ok(())
    }

    /// Get all messages for agent
    pub fn get_messages(&self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let iter = self.db.prefix_iterator(keys::INBOX_PREFIX);
        
        for item in iter {
            let (_key, value) = item?;
            if let Ok(msg) = serde_json::from_slice::<Message>(&value) {
                messages.push(msg);
            }
        }

        // Sort by timestamp descending
        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(messages)
    }

    /// Get messages from specific sender
    pub fn get_messages_from(&self, from: &str) -> Result<Vec<Message>> {
        let prefix = format!("{}{}:", keys::INBOX_PREFIX, from);
        let mut messages = Vec::new();
        let iter = self.db.prefix_iterator(prefix.as_bytes());
        
        for item in iter {
            let (key, value) = item?;
            // Check if key still matches our prefix (prefix_iterator may return more)
            if key.starts_with(prefix.as_bytes()) {
                if let Ok(msg) = serde_json::from_slice::<Message>(&value) {
                    messages.push(msg);
                }
            }
        }

        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(messages)
    }

    /// Delete message by ID
    pub fn delete_message(&self, from: &str, id: &str) -> Result<()> {
        let key = format!("{}{}:{}", keys::INBOX_PREFIX, from, id);
        self.db.delete(key.as_bytes())?;
        Ok(())
    }

    /// Clear all messages
    pub fn clear_inbox(&self) -> Result<()> {
        let iter = self.db.prefix_iterator(keys::INBOX_PREFIX);
        let keys_to_delete: Vec<Vec<u8>> = iter
            .filter_map(|item| item.ok())
            .map(|(key, _)| key.to_vec())
            .collect();

        for key in keys_to_delete {
            self.db.delete(&key)?;
        }
        Ok(())
    }

    /// Store generic key-value
    pub fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        self.db.put(key.as_bytes(), value)?;
        Ok(())
    }

    /// Get generic key-value
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.db.get(key.as_bytes())?)
    }

    /// Delete generic key
    pub fn delete(&self, key: &str) -> Result<()> {
        self.db.delete(key.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_storage_identity() {
        let dir = tempdir().unwrap();
        let storage = AgentStorage::open(dir.path()).unwrap();
        
        let identity = AgentIdentity::new("test.near".to_string(), vec![1, 2, 3]);
        storage.save_identity(&identity).unwrap();
        
        let loaded = storage.load_identity().unwrap().unwrap();
        assert_eq!(loaded.account_id, "test.near");
    }

    #[test]
    fn test_storage_messages() {
        use crate::types::{EncryptedPayload, MessageType};
        
        let dir = tempdir().unwrap();
        let storage = AgentStorage::open(dir.path()).unwrap();
        
        let msg = Message {
            id: uuid::Uuid::new_v4(),
            from: "sender.near".to_string(),
            to: "receiver.near".to_string(),
            timestamp: 12345,
            payload: EncryptedPayload {
                ciphertext: vec![],
                nonce: vec![],
                signature: vec![],
                sender_pubkey: vec![],
            },
            message_type: MessageType::Direct,
        };
        
        storage.save_message(&msg).unwrap();
        let messages = storage.get_messages().unwrap();
        assert_eq!(messages.len(), 1);
    }
}
