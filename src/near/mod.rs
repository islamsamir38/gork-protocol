use anyhow::Result;
use std::path::PathBuf;

use crate::types::AgentIdentity;

/// NEAR network configuration
#[derive(Debug, Clone)]
pub enum Network {
    Testnet,
    Mainnet,
}

impl Network {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "mainnet" => Network::Mainnet,
            _ => Network::Testnet,
        }
    }

    pub fn helper_url(&self) -> &str {
        match self {
            Network::Testnet => "https://helper.testnet.near.org",
            Network::Mainnet => "https://helper.mainnet.near.org",
        }
    }

    pub fn rpc_url(&self) -> &str {
        match self {
            Network::Testnet => "https://rpc.testnet.near.org",
            Network::Mainnet => "https://rpc.mainnet.near.org",
        }
    }
}

/// NEAR identity manager
pub struct NearIdentity {
    pub account_id: String,
    pub network: Network,
    pub credentials_path: PathBuf,
}

impl NearIdentity {
    /// Create new NEAR identity reference
    pub fn new(account_id: String, network: Network) -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let network_dir = match network {
            Network::Testnet => "testnet",
            Network::Mainnet => "mainnet",
        };
        
        let credentials_path = PathBuf::from(home)
            .join(".near-credentials")
            .join(network_dir)
            .join(format!("{}.json", account_id));

        Self {
            account_id,
            network,
            credentials_path,
        }
    }

    /// Check if credentials exist
    pub fn has_credentials(&self) -> bool {
        self.credentials_path.exists()
    }

    /// Load credentials
    pub fn load_credentials(&self) -> Result<NearCredentials> {
        let data = std::fs::read_to_string(&self.credentials_path)?;
        let creds: NearCredentials = serde_json::from_str(&data)?;
        Ok(creds)
    }

    /// Get agent identity from NEAR account
    pub fn to_agent_identity(&self, public_key: Vec<u8>) -> AgentIdentity {
        AgentIdentity::new(self.account_id.clone(), public_key)
    }

    /// Validate account exists on network
    pub async fn validate_account(&self) -> Result<bool> {
        let client = reqwest::Client::new();
        let url = self.network.rpc_url();
        
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "view_account",
                "finality": "final",
                "account_id": self.account_id
            }
        });

        let resp = client
            .post(url)
            .json(&body)
            .send()
            .await?;

        let result: serde_json::Value = resp.json().await?;
        
        // Check if account exists
        Ok(result.get("result").is_some())
    }
}

/// NEAR credentials from ~/.near-credentials
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NearCredentials {
    pub account_id: String,
    pub public_key: String,
    pub private_key: String,
}

/// Check if NEAR CLI is configured
pub fn is_near_configured() -> bool {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let config_path = PathBuf::from(home).join(".near-credentials");
    config_path.exists()
}

/// Get default credentials path
pub fn default_credentials_path(network: &Network) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let network_dir = match network {
        Network::Testnet => "testnet",
        Network::Mainnet => "mainnet",
    };
    PathBuf::from(home).join(".near-credentials").join(network_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_urls() {
        let testnet = Network::Testnet;
        assert!(testnet.rpc_url().contains("testnet"));
        
        let mainnet = Network::Mainnet;
        assert!(mainnet.rpc_url().contains("mainnet"));
    }

    #[test]
    fn test_identity_creation() {
        let identity = NearIdentity::new("test.near".to_string(), Network::Testnet);
        assert_eq!(identity.account_id, "test.near");
        assert!(!identity.has_credentials()); // Won't exist in test
    }
}
