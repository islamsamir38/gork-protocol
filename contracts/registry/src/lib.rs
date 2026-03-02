use near_sdk::{env, near, AccountId, BorshStorageKey, require, collections::UnorderedMap, collections::UnorderedSet};
use borsh::{BorshDeserialize, BorshSerialize};
use borsh::BorshSchema;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Storage keys for the contract
#[derive(BorshSerialize, BorshDeserialize, BorshStorageKey)]
pub enum StorageKey {
    Agents,
    AgentsKeys,
    OnlineAgents,
    OnlineAgentsKeys,
}

/// Agent metadata stored on-chain
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, BorshSchema, JsonSchema,
)]
pub struct AgentMetadata {
    /// NEAR account ID
    pub account_id: String,
    /// Human-readable name
    pub name: String,
    /// List of capabilities (e.g., ["trading", "monitoring", "analysis"])
    pub capabilities: Vec<String>,
    /// P2P endpoint (multiaddr format)
    pub endpoint: Option<String>,
    /// X25519 public key for encryption (base58 encoded)
    pub public_key: String,
    /// Reputation score (0-100)
    pub reputation: u32,
    /// Total ratings received
    pub rating_count: u32,
    /// Last seen timestamp (nanoseconds)
    pub last_seen: u64,
    /// Agent description
    pub description: String,
    /// Is agent currently online
    pub online: bool,
}

#[near(contract_state)]
pub struct AgentRegistry {
    /// All registered agents by account ID
    agents: UnorderedMap<AccountId, AgentMetadata>,
    /// Set of online agents for quick lookup
    online_agents: UnorderedSet<AccountId>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self {
            agents: UnorderedMap::new(StorageKey::Agents),
            online_agents: UnorderedSet::new(StorageKey::OnlineAgents),
        }
    }
}

#[near]
impl AgentRegistry {
    /// Register or update agent metadata
    /// 
    /// # Arguments
    /// * `name` - Human-readable name for the agent
    /// * `capabilities` - List of capabilities the agent provides
    /// * `endpoint` - Optional P2P endpoint (multiaddr format)
    /// * `public_key` - X25519 public key for encryption (base58)
    /// * `description` - Agent description
    pub fn register(
        &mut self,
        name: String,
        capabilities: Vec<String>,
        endpoint: Option<String>,
        public_key: String,
        description: String,
    ) -> bool {
        let account_id = env::signer_account_id();
        let now = env::block_timestamp();

        // Check if updating existing agent
        let (reputation, rating_count) = match self.agents.get(&account_id) {
            Some(existing) => (existing.reputation, existing.rating_count),
            None => (50, 0), // Default reputation for new agents
        };

        let metadata = AgentMetadata {
            account_id: account_id.to_string(),
            name,
            capabilities,
            endpoint,
            public_key,
            reputation,
            rating_count,
            last_seen: now,
            description,
            online: true,
        };

        self.agents.insert(&account_id, &metadata);
        self.online_agents.insert(&account_id);

        near_sdk::log!("Agent registered: {}", account_id);
        true
    }

    /// Update agent online status (heartbeat)
    pub fn heartbeat(&mut self) -> bool {
        let account_id = env::signer_account_id();
        
        if let Some(mut metadata) = self.agents.get(&account_id) {
            metadata.last_seen = env::block_timestamp();
            metadata.online = true;
            self.agents.insert(&account_id, &metadata);
            self.online_agents.insert(&account_id);
            near_sdk::log!("Heartbeat received from: {}", account_id);
            true
        } else {
            false
        }
    }

    /// Set agent offline
    pub fn set_offline(&mut self) -> bool {
        let account_id = env::signer_account_id();
        
        if let Some(mut metadata) = self.agents.get(&account_id) {
            metadata.online = false;
            self.agents.insert(&account_id, &metadata);
            self.online_agents.remove(&account_id);
            near_sdk::log!("Agent offline: {}", account_id);
            true
        } else {
            false
        }
    }

    /// Get specific agent by account ID
    pub fn get_agent(&self, account_id: AccountId) -> Option<AgentMetadata> {
        self.agents.get(&account_id)
    }

    /// Discover agents by capability
    /// 
    /// # Arguments
    /// * `capability` - Capability to search for (e.g., "trading")
    /// * `online_only` - Only return online agents
    /// * `limit` - Maximum number of results
    pub fn discover(
        &self,
        capability: String,
        online_only: bool,
        limit: Option<u32>,
    ) -> Vec<AgentMetadata> {
        let limit = limit.unwrap_or(20) as usize;
        let mut results = Vec::new();

        // Iterate through all agents
        for (account_id, metadata) in self.agents.iter() {
            if metadata.capabilities.contains(&capability) {
                if !online_only || metadata.online {
                    results.push(metadata.clone());
                    if results.len() >= limit {
                        break;
                    }
                }
            }
            let _ = account_id; // Suppress unused warning
        }

        // Sort by reputation (descending)
        results.sort_by(|a, b| b.reputation.cmp(&a.reputation));
        results
    }

    /// Get all agents (paginated)
    pub fn get_all_agents(&self, from_index: Option<u64>, limit: Option<u64>) -> Vec<AgentMetadata> {
        let from = from_index.unwrap_or(0) as usize;
        let limit = limit.unwrap_or(50) as usize;

        self.agents
            .iter()
            .skip(from)
            .take(limit)
            .map(|(_, metadata)| metadata)
            .collect()
    }

    /// Get online agents count
    pub fn get_online_count(&self) -> u32 {
        self.online_agents.len() as u32
    }

    /// Get total agents count
    pub fn get_total_count(&self) -> u32 {
        self.agents.len() as u32
    }

    /// Rate an agent (increases reputation)
    /// 
    /// # Arguments
    /// * `agent_id` - Agent to rate
    /// * `score` - Score from 1-100
    /// 
    /// # Requirements
    /// - Caller must be a registered agent
    /// - Score must be 1-100
    pub fn rate_agent(&mut self, agent_id: AccountId, score: u32) -> bool {
        let caller = env::signer_account_id();
        
        // Caller must be registered
        require!(self.agents.get(&caller).is_some(), "Caller not registered");
        
        // Can't rate yourself
        require!(caller != agent_id, "Cannot rate yourself");
        
        // Score must be valid
        require!(score >= 1 && score <= 100, "Score must be 1-100");

        if let Some(mut metadata) = self.agents.get(&agent_id) {
            // Calculate new reputation (weighted average)
            let total_score = metadata.reputation * metadata.rating_count + score;
            metadata.rating_count += 1;
            metadata.reputation = total_score / metadata.rating_count;
            
            // Cap at 100
            metadata.reputation = metadata.reputation.min(100);
            
            self.agents.insert(&agent_id, &metadata);
            near_sdk::log!("Agent {} rated {} by {}", agent_id, score, caller);
            true
        } else {
            false
        }
    }

    /// Unregister agent
    pub fn unregister(&mut self) -> bool {
        let account_id = env::signer_account_id();
        
        if self.agents.remove(&account_id).is_some() {
            self.online_agents.remove(&account_id);
            near_sdk::log!("Agent unregistered: {}", account_id);
            true
        } else {
            false
        }
    }

    /// Update agent capabilities
    pub fn update_capabilities(&mut self, capabilities: Vec<String>) -> bool {
        let account_id = env::signer_account_id();
        
        if let Some(mut metadata) = self.agents.get(&account_id) {
            metadata.capabilities = capabilities;
            metadata.last_seen = env::block_timestamp();
            self.agents.insert(&account_id, &metadata);
            true
        } else {
            false
        }
    }

    /// Update agent endpoint
    pub fn update_endpoint(&mut self, endpoint: Option<String>) -> bool {
        let account_id = env::signer_account_id();
        
        if let Some(mut metadata) = self.agents.get(&account_id) {
            metadata.endpoint = endpoint;
            metadata.last_seen = env::block_timestamp();
            self.agents.insert(&account_id, &metadata);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;

    fn get_context(is_view: bool) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(0))
            .signer_account_id(accounts(1))
            .predecessor_account_id(accounts(1))
            .is_view(is_view);
        builder
    }

    #[test]
    fn test_register_agent() {
        let mut context = get_context(false);
        testing_env!(context.build());

        let mut contract = AgentRegistry::default();

        let result = contract.register(
            "Test Agent".to_string(),
            vec!["trading".to_string(), "monitoring".to_string()],
            Some("/ip4/1.2.3.4/tcp/4001".to_string()),
            "base58publickey".to_string(),
            "A test agent".to_string(),
        );

        assert!(result);
        assert_eq!(contract.get_total_count(), 1);
    }

    #[test]
    fn test_discover_by_capability() {
        let mut context = get_context(false);
        testing_env!(context.build());

        let mut contract = AgentRegistry::default();

        // Register agent 1
        testing_env!(context.signer_account_id(accounts(1)).build());
        contract.register(
            "Agent 1".to_string(),
            vec!["trading".to_string()],
            None,
            "key1".to_string(),
            "Trading agent".to_string(),
        );

        // Register agent 2
        testing_env!(context.signer_account_id(accounts(2)).build());
        contract.register(
            "Agent 2".to_string(),
            vec!["analysis".to_string()],
            None,
            "key2".to_string(),
            "Analysis agent".to_string(),
        );

        // Discover trading agents
        testing_env!(context.is_view(true).build());
        let results = contract.discover("trading".to_string(), false, Some(10));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Agent 1");
    }

    #[test]
    fn test_rate_agent() {
        let mut context = get_context(false);
        testing_env!(context.build());

        let mut contract = AgentRegistry::default();

        // Register agent 1
        testing_env!(context.signer_account_id(accounts(1)).build());
        contract.register(
            "Agent 1".to_string(),
            vec!["trading".to_string()],
            None,
            "key1".to_string(),
            "Trading agent".to_string(),
        );

        // Register agent 2
        testing_env!(context.signer_account_id(accounts(2)).build());
        contract.register(
            "Agent 2".to_string(),
            vec!["trading".to_string()],
            None,
            "key2".to_string(),
            "Trading agent".to_string(),
        );

        // Agent 1 rates Agent 2
        contract.rate_agent(accounts(1), 80);

        // Check reputation
        testing_env!(context.is_view(true).build());
        let agent = contract.get_agent(accounts(1)).unwrap();
        assert_eq!(agent.reputation, 80);
    }
}
