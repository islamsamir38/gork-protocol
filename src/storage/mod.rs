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
                sender_pubkey BLOB,
                delivered_at INTEGER,
                delivery_status TEXT DEFAULT 'pending'
            );
            
            CREATE INDEX IF NOT EXISTS idx_messages_sender ON messages(sender);
            CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp);
            
            CREATE TABLE IF NOT EXISTS api_keys (
                key TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                last_used INTEGER,
                permissions TEXT NOT NULL DEFAULT 'read,write'
            );
            
            CREATE TABLE IF NOT EXISTS message_queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                to_account TEXT NOT NULL,
                message TEXT NOT NULL,
                queued_at INTEGER NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                last_attempt INTEGER,
                status TEXT NOT NULL DEFAULT 'pending'
            );
            
            CREATE INDEX IF NOT EXISTS idx_queue_status ON message_queue(status);",
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
    
    /// Mark message as delivered
    pub fn mark_delivered(&self, message_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "UPDATE messages SET delivered_at = ?1, delivery_status = 'delivered' WHERE id = ?2",
            params![now, message_id],
        )?;
        Ok(())
    }
    
    /// Get delivery status for message
    pub fn get_delivery_status(&self, message_id: &str) -> Result<Option<(i64, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT delivered_at, delivery_status FROM messages WHERE id = ?1"
        )?;
        let result = stmt.query_row(params![message_id], |row| {
            let delivered_at: i64 = row.get(0)?;
            let status: String = row.get(1)?;
            Ok((delivered_at, status))
        });
        
        match result {
            Ok(data) => Ok(Some(data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
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
    
    // ========================================================================
    // API Key Management
    // ========================================================================
    
    /// Generate new API key
    pub fn create_api_key(&self, name: &str, permissions: &str) -> Result<String> {
        let key = format!("gork_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
        let created_at = chrono::Utc::now().timestamp();
        
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO api_keys (key, name, created_at, permissions) VALUES (?1, ?2, ?3, ?4)",
            params![key, name, created_at, permissions],
        )?;
        
        Ok(key)
    }
    
    /// Validate API key
    pub fn validate_api_key(&self, key: &str, required_permission: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT permissions, last_used FROM api_keys WHERE key = ?1"
        )?;
        
        let result = stmt.query_row(params![key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
        });
        
        match result {
            Ok((permissions, _last_used)) => {
                // Check if key has required permission
                let has_permission = permissions.split(',')
                    .map(|s| s.trim())
                    .any(|p| p == required_permission || p == "admin");
                
                if has_permission {
                    // Update last_used
                    let now = chrono::Utc::now().timestamp();
                    conn.execute(
                        "UPDATE api_keys SET last_used = ?1 WHERE key = ?2",
                        params![now, key],
                    )?;
                }
                
                Ok(has_permission)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
    
    /// List all API keys
    pub fn list_api_keys(&self) -> Result<Vec<(String, String, i64, Option<i64>)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT key, name, created_at, last_used FROM api_keys ORDER BY created_at DESC"
        )?;
        
        let keys = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?.collect::<Result<Vec<_>, _>>()?;
        
        Ok(keys)
    }
    
    /// Revoke API key
    pub fn revoke_api_key(&self, key: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM api_keys WHERE key = ?1", params![key])?;
        Ok(rows > 0)
    }
    
    // ========================================================================
    // Message Queue (for offline sending)
    // ========================================================================
    
    /// Queue message for later delivery
    pub fn queue_message(&self, to_account: &str, message: &str) -> Result<i64> {
        let queued_at = chrono::Utc::now().timestamp();
        
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO message_queue (to_account, message, queued_at, status) VALUES (?1, ?2, ?3, 'pending')",
            params![to_account, message, queued_at],
        )?;
        
        Ok(conn.last_insert_rowid())
    }
    
    /// Get pending messages from queue
    pub fn get_pending_messages(&self, limit: usize) -> Result<Vec<(i64, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, to_account, message FROM message_queue WHERE status = 'pending' ORDER BY queued_at ASC LIMIT ?1"
        )?;
        
        let messages = stmt.query_map(params![limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?.collect::<Result<Vec<_>, _>>()?;
        
        Ok(messages)
    }
    
    /// Mark message as sent
    pub fn mark_message_sent(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE message_queue SET status = 'sent' WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }
    
    /// Increment attempt counter
    pub fn increment_attempts(&self, id: i64) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE message_queue SET attempts = attempts + 1, last_attempt = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        
        let attempts: u32 = conn.query_row(
            "SELECT attempts FROM message_queue WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        
        Ok(attempts)
    }
    
    /// Clean up old sent messages (older than days)
    pub fn cleanup_queue(&self, days: i64) -> Result<usize> {
        let cutoff = chrono::Utc::now().timestamp() - (days * 86400);
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM message_queue WHERE status = 'sent' AND queued_at < ?1",
            params![cutoff],
        )?;
        Ok(rows)
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
