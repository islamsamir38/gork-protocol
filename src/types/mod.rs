use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type MessageId = Uuid;

/// Agent identity (NEAR-native)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub account_id: String,
    pub public_key: Vec<u8>,
    pub capabilities: Vec<String>,
    pub endpoint: Option<String>,
    pub created_at: u64,
}

impl AgentIdentity {
    pub fn new(account_id: String, public_key: Vec<u8>) -> Self {
        Self {
            account_id,
            public_key,
            capabilities: Vec::new(),
            endpoint: None,
            created_at: chrono::Utc::now().timestamp_millis() as u64,
        }
    }

    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }
}

/// Message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub from: String,
    pub to: String,
    pub timestamp: u64,
    pub payload: EncryptedPayload,
    pub message_type: MessageType,
}

impl Message {
    pub fn new(from: String, to: String, payload: EncryptedPayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            from,
            to,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            payload,
            message_type: MessageType::Direct,
        }
    }
}

/// Message types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    Direct,
    Broadcast,
    Request,
    Response,
}

/// Encrypted message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub signature: Vec<u8>,
    pub sender_pubkey: Vec<u8>,
}

/// Plain text message (for internal use before encryption)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlainMessage {
    pub content: String,
    pub timestamp: u64,
}

impl PlainMessage {
    pub fn new(content: String) -> Self {
        Self {
            content,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("Failed to serialize message")
    }

    pub fn from_bytes(data: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(data)?)
    }
}

/// Capability request (agent-to-agent)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub request_id: String,
    pub capability: String,
    pub params: serde_json::Value,
    pub timeout_ms: u64,
    pub reward: Option<String>,
}

/// Capability response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityResponse {
    pub request_id: String,
    pub result: Result<serde_json::Value, String>,
    pub execution_time_ms: u64,
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub identity: AgentIdentity,
    pub storage_path: String,
    pub network_id: String,
    /// Whether this agent was initialized with NEAR verification
    pub near_verified: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            identity: AgentIdentity::new(String::new(), Vec::new()),
            storage_path: ".gork-agent".to_string(),
            network_id: "testnet".to_string(),
            near_verified: false,
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_identity_creation() {
        let identity = AgentIdentity::new("test.near".to_string(), vec![1, 2, 3, 4]);
        assert_eq!(identity.account_id, "test.near");
        assert_eq!(identity.public_key, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_agent_identity_with_capabilities() {
        let identity = AgentIdentity::new("test.near".to_string(), vec![])
            .with_capabilities(vec!["storage".to_string(), "compute".to_string()]);
        assert_eq!(identity.capabilities.len(), 2);
    }

    #[test]
    fn test_capability_request_creation() {
        let request = CapabilityRequest {
            request_id: "req-1".to_string(),
            capability: "hash".to_string(),
            params: serde_json::json!({"data": "test"}),
            timeout_ms: 30000,
            reward: Some("1 NEAR".to_string()),
        };
        assert_eq!(request.capability, "hash");
    }

    #[test]
    fn test_capability_response_success() {
        let response = CapabilityResponse {
            request_id: "req-1".to_string(),
            result: Ok(serde_json::json!({"hash": "abc123"})),
            execution_time_ms: 500,
        };
        assert!(response.result.is_ok());
    }

    #[test]
    fn test_capability_response_error() {
        let response = CapabilityResponse {
            request_id: "req-1".to_string(),
            result: Err("Timeout".to_string()),
            execution_time_ms: 30000,
        };
        assert!(response.result.is_err());
    }

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.network_id, "testnet");
        assert!(!config.near_verified);
    }

    #[test]
    fn test_message_creation() {
        let plain = PlainMessage::new("Hello".to_string());
        assert_eq!(plain.content, "Hello");
    }

    #[test]
    fn test_plain_message_bytes() {
        let plain = PlainMessage::new("Test".to_string());
        let bytes = plain.to_bytes();
        assert!(!bytes.is_empty());
    }
}
