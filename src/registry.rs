use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Agent metadata from registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub account_id: String,
    pub name: String,
    pub capabilities: Vec<String>,
    pub endpoint: Option<String>,
    pub public_key: String,
    pub reputation: u32,
    pub rating_count: u32,
    pub last_seen: u64,
    pub description: String,
    pub online: bool,
}

/// NEAR RPC client for registry contract
pub struct RegistryClient {
    rpc_url: String,
    contract_id: String,
}

impl RegistryClient {
    pub fn new(contract_id: String, network: &str) -> Self {
        let rpc_url = match network {
            "mainnet" => "https://rpc.mainnet.near.org".to_string(),
            _ => "https://rpc.testnet.near.org".to_string(),
        };
        Self { rpc_url, contract_id }
    }

    /// Call a view method on the registry
    pub async fn view<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        args: serde_json::Value,
    ) -> Result<T> {
        let args_base64 = base64::encode(serde_json::to_vec(&args)?);
        
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "call_function",
                "finality": "final",
                "account_id": self.contract_id,
                "method_name": method,
                "args_base64": args_base64
            }
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        // Parse result
        let result = resp.get("result")
            .ok_or_else(|| anyhow::anyhow!("No result in response"))?;
        
        let result_data = result.get("result")
            .ok_or_else(|| anyhow::anyhow!("No result data"))?;
        
        // Handle bytes array
        let bytes: Vec<u8> = serde_json::from_value(result_data.clone())?;
        let json_str = String::from_utf8(bytes)?;
        let parsed: T = serde_json::from_str(&json_str)?;
        
        Ok(parsed)
    }

    /// Get agent by account ID
    pub async fn get_agent(&self, account_id: &str) -> Result<Option<AgentMetadata>> {
        let args = serde_json::json!({ "account_id": account_id });
        self.view("get_agent", args).await
    }

    /// Discover agents by capability
    pub async fn discover(
        &self,
        capability: &str,
        online_only: bool,
        limit: u32,
    ) -> Result<Vec<AgentMetadata>> {
        let args = serde_json::json!({
            "capability": capability,
            "online_only": online_only,
            "limit": limit
        });
        self.view("discover", args).await
    }

    /// Get all agents
    pub async fn get_all_agents(&self, from_index: u64, limit: u64) -> Result<Vec<AgentMetadata>> {
        let args = serde_json::json!({
            "from_index": from_index,
            "limit": limit
        });
        self.view("get_all_agents", args).await
    }

    /// Get total agent count
    pub async fn get_total_count(&self) -> Result<u32> {
        let args = serde_json::json!({});
        self.view("get_total_count", args).await
    }

    /// Get online agent count
    pub async fn get_online_count(&self) -> Result<u32> {
        let args = serde_json::json!({});
        self.view("get_online_count", args).await
    }
}
