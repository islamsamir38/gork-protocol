use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::types::{AgentIdentity, Message, AgentConfig};

/// Persistent storage for agent data using SQLite
/// SQLite supports concurrent reads, allowing daemon and CLI to coexist
pub struct AgentStorage {
    conn: Arc<Mutex<Connection>>,
}

impl AgentStorage {
    /// Open or create storage at path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let db_path = path.as_ref().join("agent.db");
        let conn = Connection::open(&db_path)?;
        
        // Enable WAL mode for better concurrency
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        conn.pragma_update(None, "synchronous", &"NORMAL")?;
        conn.pragma_update(None, "busy_timeout", &5000)?; // 5 second timeout
        
        // Create tables
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                sender TEXT NOT NULL,
                receiver TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                message_type TEXT NOT NULL,
                ciphertext BLOB,
                nonce BLOB,
                signature BLOB,
                sender_pubkey BLOB
            );
            
            CREATE INDEX IF NOT EXISTS idx_messages_sender ON messages(sender);
            CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp);",
        )?;
        
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Save agent identity
    pub fn save_identity(&self, identity: &AgentIdentity) -> Result<()> {
        let data = serde_json::to_vec(identity)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value) VALUES (?1, ?2)",
            params!["identity", data],
        )?;
        Ok(())
    }

    /// Load agent identity
    pub fn load_identity(&self) -> Result<Option<AgentIdentity>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM kv_store WHERE key = ?1")?;
        let result = stmt.query_row(params!["identity"], |row| {
            let value: Vec<u8> = row.get(0)?;
            Ok(value)
        });
        
        match result {
            Ok(data) => Ok(Some(serde_json::from_slice(&data)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Save agent config
    pub fn save_config(&self, config: &AgentConfig) -> Result<()> {
        let data = serde_json::to_vec(config)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value) VALUES (?1, ?2)",
            params!["config", data],
        )?;
        Ok(())
    }

    /// Load agent config
    pub fn load_config(&self) -> Result<Option<AgentConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM kv_store WHERE key = ?1")?;
        let result = stmt.query_row(params!["config"], |row| {
            let value: Vec<u8> = row.get(0)?;
            Ok(value)
        });
        
        match result {
            Ok(data) => Ok(Some(serde_json::from_slice(&data)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Save agent certificate
    pub fn save_certificate(&self, cert: &crate::certificate::AgentCertificate) -> Result<()> {
        let data = serde_json::to_vec(cert)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value) VALUES (?1, ?2)",
            params!["certificate", data],
        )?;
        Ok(())
    }

    /// Load agent certificate
    pub fn load_certificate(&self) -> Result<Option<crate::certificate::AgentCertificate>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM kv_store WHERE key = ?1")?;
        let result = stmt.query_row(params!["certificate"], |row| {
            let value: Vec<u8> = row.get(0)?;
            Ok(value)
        });
        
        match result {
            Ok(data) => Ok(Some(serde_json::from_slice(&data)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Save agent keypair (encrypted in production)
    pub fn save_agent_keypair(&self, _crypto: &crate::crypto::MessageCrypto) -> Result<()> {
        // TODO: Encrypt private key before storing
        // For now, the keypair is stored in memory only
        Ok(())
    }

    /// Save message to inbox
    pub fn save_message(&self, message: &Message) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO messages 
             (id, sender, receiver, timestamp, message_type, ciphertext, nonce, signature, sender_pubkey)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                message.id.to_string(),
                message.from,
                message.to,
                message.timestamp as i64,
                format!("{:?}", message.message_type),
                message.payload.ciphertext,
                message.payload.nonce,
                message.payload.signature,
                message.payload.sender_pubkey,
            ],
        )?;
        Ok(())
    }

    /// Get all messages for agent
    pub fn get_messages(&self) -> Result<Vec<Message>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, sender, receiver, timestamp, message_type, ciphertext, nonce, signature, sender_pubkey
             FROM messages ORDER BY timestamp DESC"
        )?;
        
        let messages = stmt.query_map([], |row| {
            Ok(Message {
                id: uuid::Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                from: row.get(1)?,
                to: row.get(2)?,
                timestamp: row.get::<_, i64>(3)? as u64,
                payload: crate::types::EncryptedPayload {
                    ciphertext: row.get(5)?,
                    nonce: row.get(6)?,
                    signature: row.get(7)?,
                    sender_pubkey: row.get(8)?,
                },
                message_type: match row.get::<_, String>(4)?.as_str() {
                    "Direct" => crate::types::MessageType::Direct,
                    "Broadcast" => crate::types::MessageType::Broadcast,
                    _ => crate::types::MessageType::Direct,
                },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(messages)
    }

    /// Get messages from specific sender
    pub fn get_messages_from(&self, from: &str) -> Result<Vec<Message>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, sender, receiver, timestamp, message_type, ciphertext, nonce, signature, sender_pubkey
             FROM messages WHERE sender = ?1 ORDER BY timestamp DESC"
        )?;
        
        let messages = stmt.query_map(params![from], |row| {
            Ok(Message {
                id: uuid::Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                from: row.get(1)?,
                to: row.get(2)?,
                timestamp: row.get::<_, i64>(3)? as u64,
                payload: crate::types::EncryptedPayload {
                    ciphertext: row.get(5)?,
                    nonce: row.get(6)?,
                    signature: row.get(7)?,
                    sender_pubkey: row.get(8)?,
                },
                message_type: match row.get::<_, String>(4)?.as_str() {
                    "Direct" => crate::types::MessageType::Direct,
                    "Broadcast" => crate::types::MessageType::Broadcast,
                    _ => crate::types::MessageType::Direct,
                },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(messages)
    }

    /// Delete message by ID
    pub fn delete_message(&self, from: &str, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM messages WHERE sender = ?1 AND id = ?2",
            params![from, id],
        )?;
        Ok(())
    }

    /// Clear all messages
    pub fn clear_inbox(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM messages", [])?;
        Ok(())
    }

    /// Store generic key-value
    pub fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Get generic key-value
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM kv_store WHERE key = ?1")?;
        let result = stmt.query_row(params![key], |row| {
            let value: Vec<u8> = row.get(0)?;
            Ok(value)
        });
        
        match result {
            Ok(data) => Ok(Some(data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete generic key
    pub fn delete(&self, key: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM kv_store WHERE key = ?1", params![key])?;
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
    
    #[test]
    fn test_concurrent_access() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();
        
        // Open two connections to same database
        let storage1 = AgentStorage::open(&path).unwrap();
        let storage2 = AgentStorage::open(&path).unwrap();
        
        // Write from first
        let identity = AgentIdentity::new("test.near".to_string(), vec![1, 2, 3]);
        storage1.save_identity(&identity).unwrap();
        
        // Read from second (should work with SQLite + WAL)
        let loaded = storage2.load_identity().unwrap().unwrap();
        assert_eq!(loaded.account_id, "test.near");
    }
}
