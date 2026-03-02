use near_sdk::{env, near, AccountId, BorshStorageKey, require, collections::UnorderedMap, collections::UnorderedSet, json_types::U128};
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
    Skills,           // Agent Skills support
    SkillsByAgent,    // Index: agent -> their skills
    SkillsByTag,      // Index: tag -> skills
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

/// Skill manifest following Agent Skills format (agentskills.io)
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, JsonSchema)]
pub struct SkillManifest {
    /// Unique skill identifier (name@version)
    pub skill_id: String,
    /// Human-readable name
    pub name: String,
    /// Version string
    pub version: String,
    /// Skill author (NEAR account)
    pub author: String,  // String instead of AccountId for JSON schema
    /// Description
    pub description: String,
    /// Tags for discovery
    pub tags: Vec<String>,
    /// Detailed capabilities
    pub capabilities: Vec<CapabilityDetail>,
    /// Resource requirements
    pub requirements: ResourceRequirements,
    /// Optional pricing
    pub pricing: Option<SkillPricing>,
    /// IPFS hash for full skill package
    pub ipfs_hash: String,
    /// Checksum for verification
    pub checksum: String,
    /// Usage statistics
    pub usage_count: u32,
    /// Average rating (1.0 - 5.0)
    pub rating: f32,
    /// Total number of ratings
    pub rating_count: u32,
    /// Created at timestamp
    pub created_at: u64,
}

/// Detailed capability with JSON schemas
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, JsonSchema)]
pub struct CapabilityDetail {
    pub name: String,
    pub description: String,
    pub input_schema: String,
    pub output_schema: String,
    pub examples: Vec<String>,
}

/// Resource requirements
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, JsonSchema)]
pub struct ResourceRequirements {
    pub timeout_secs: u32,
    pub memory_mb: u32,
    pub dependencies: Vec<String>,
}

/// Pricing model
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, JsonSchema)]
pub struct SkillPricing {
    pub free_tier_calls_per_day: Option<u32>,
    pub cost_per_call_yocto: Option<String>,  // U128 as string for JSON compatibility
}

/// Skill statistics (lighter than full manifest)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillStats {
    pub skill_id: String,
    pub usage_count: u32,
    pub rating: f32,
    pub rating_count: u32,
    pub author: String,
}

#[near(contract_state)]
pub struct AgentRegistry {
    /// All registered agents by account ID
    agents: UnorderedMap<AccountId, AgentMetadata>,
    /// Set of online agents for quick lookup
    online_agents: UnorderedSet<AccountId>,
    /// All skills by skill_id (name@version)
    skills: UnorderedMap<String, SkillManifest>,
    /// Index: agent -> list of their skill_ids
    skills_by_agent: UnorderedMap<AccountId, Vec<String>>,
    /// Index: tag -> list of skill_ids with that tag
    skills_by_tag: UnorderedMap<String, Vec<String>>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self {
            agents: UnorderedMap::new(StorageKey::Agents),
            online_agents: UnorderedSet::new(StorageKey::OnlineAgents),
            skills: UnorderedMap::new(StorageKey::Skills),
            skills_by_agent: UnorderedMap::new(StorageKey::SkillsByAgent),
            skills_by_tag: UnorderedMap::new(StorageKey::SkillsByTag),
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

    // ============= AGENT SKILLS METHODS =============

    /// Register a skill (Agent Skills format)
    #[payable]
    pub fn register_skill(
        &mut self,
        name: String,
        version: String,
        description: String,
        tags: Vec<String>,
        capabilities: Vec<CapabilityDetail>,
        requirements: ResourceRequirements,
        pricing: Option<SkillPricing>,
        ipfs_hash: String,
        checksum: String,
    ) -> bool {
        let author = env::signer_account_id();
        let now = env::block_timestamp();

        // Author must be a registered agent
        require!(self.agents.get(&author).is_some(), "Author must be registered agent");

        // Create skill ID
        let skill_id = format!("{}@{}", name, version);

        // Check if updating existing skill
        let (usage_count, rating, rating_count) = match self.skills.get(&skill_id) {
            Some(existing) => {
                require!(existing.author == author.to_string(), "Only skill author can update");
                (existing.usage_count, existing.rating, existing.rating_count)
            }
            None => (0, 5.0, 0),
        };

        let manifest = SkillManifest {
            skill_id: skill_id.clone(),
            name: name.clone(),
            version,
            author: author.to_string(),
            description,
            tags: tags.clone(),
            capabilities,
            requirements,
            pricing,
            ipfs_hash,
            checksum,
            usage_count,
            rating,
            rating_count,
            created_at: now,
        };

        // Store skill
        self.skills.insert(&skill_id, &manifest);

        // Index by agent
        let mut agent_skills = self.skills_by_agent.get(&author).unwrap_or_default();
        if !agent_skills.contains(&skill_id) {
            agent_skills.push(skill_id.clone());
            self.skills_by_agent.insert(&author, &agent_skills);
        }

        // Index by tags
        for tag in &tags {
            let mut tag_skills = self.skills_by_tag.get(tag).unwrap_or_default();
            if !tag_skills.contains(&skill_id) {
                tag_skills.push(skill_id.clone());
                self.skills_by_tag.insert(tag, &tag_skills);
            }
        }

        near_sdk::log!("Skill registered: {} by {}", skill_id, author);
        true
    }

    /// Get skill manifest by ID
    pub fn get_skill(&self, skill_id: String) -> Option<SkillManifest> {
        self.skills.get(&skill_id)
    }

    /// Discover skills by tag
    pub fn discover_skills(
        &self,
        tag: String,
        min_rating: Option<f32>,
        limit: Option<u32>,
    ) -> Vec<SkillManifest> {
        let limit = limit.unwrap_or(20) as usize;
        let min_rating = min_rating.unwrap_or(0.0);

        self.skills_by_tag.get(&tag)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|skill_id| self.skills.get(&skill_id))
            .filter(|skill| skill.rating >= min_rating)
            .take(limit)
            .collect()
    }

    /// Search skills by name/description
    pub fn search_skills(&self, query: String, limit: Option<u32>) -> Vec<SkillManifest> {
        let limit = limit.unwrap_or(20) as usize;
        let query_lower = query.to_lowercase();

        self.skills.values()
            .filter(|skill| {
                skill.name.to_lowercase().contains(&query_lower) ||
                skill.description.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .collect()
    }

    /// Get all skills for a specific agent
    pub fn get_agent_skills(&self, agent_id: AccountId) -> Vec<SkillManifest> {
        self.skills_by_agent.get(&agent_id)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|skill_id| self.skills.get(&skill_id))
            .collect()
    }

    /// Find agents that have a specific skill
    pub fn find_agents_with_skill(&self, skill_name: String) -> Vec<AgentMetadata> {
        self.skills.values()
            .filter(|skill| skill.name == skill_name)
            .filter_map(|skill| {
                // Parse author string as AccountId
                skill.author.parse::<AccountId>().ok()
                    .and_then(|author_id| self.agents.get(&author_id))
            })
            .collect()
    }

    /// Rate a skill (1-5 stars)
    pub fn rate_skill(&mut self, skill_id: String, rating: u32) -> bool {
        let caller = env::signer_account_id();

        require!(self.agents.get(&caller).is_some(), "Caller not registered");
        require!(rating >= 1 && rating <= 5, "Rating must be 1-5");

        if let Some(mut skill) = self.skills.get(&skill_id) {
            require!(skill.author != caller.to_string(), "Cannot rate your own skill");

            let total = skill.rating * skill.rating_count as f32 + rating as f32;
            skill.rating_count += 1;
            skill.rating = total / skill.rating_count as f32;

            self.skills.insert(&skill_id, &skill);
            near_sdk::log!("Skill {} rated {} by {}", skill_id, rating, caller);
            true
        } else {
            false
        }
    }

    /// Track skill usage
    pub fn track_skill_usage(&mut self, skill_id: String) -> bool {
        if let Some(mut skill) = self.skills.get(&skill_id) {
            skill.usage_count += 1;
            self.skills.insert(&skill_id, &skill);
            true
        } else {
            false
        }
    }

    /// Get skill statistics
    pub fn get_skill_stats(&self, skill_id: String) -> Option<SkillStats> {
        self.skills.get(&skill_id).map(|skill| SkillStats {
            skill_id: skill.skill_id.clone(),
            usage_count: skill.usage_count,
            rating: skill.rating,
            rating_count: skill.rating_count,
            author: skill.author.to_string(),
        })
    }

    /// Get top skills by usage
    pub fn get_top_skills(&self, limit: Option<u32>) -> Vec<SkillStats> {
        let limit = limit.unwrap_or(10) as usize;

        let mut skills: Vec<_> = self.skills.iter()
            .map(|(id, skill)| SkillStats {
                skill_id: id.clone(),
                usage_count: skill.usage_count,
                rating: skill.rating,
                rating_count: skill.rating_count,
                author: skill.author.to_string(),
            })
            .collect();
        skills.sort_by(|a, b| b.usage_count.cmp(&a.usage_count));
        skills.into_iter().take(limit).collect()
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

    #[test]
    fn test_register_skill() {
        let mut context = get_context(false);
        testing_env!(context.build());

        let mut contract = AgentRegistry::default();

        // Register agent first
        contract.register(
            "Test Agent".to_string(),
            vec!["trading".to_string()],
            None,
            "publickey".to_string(),
            "Test agent".to_string(),
        );

        // Register a skill
        let result = contract.register_skill(
            "csv-analyzer".to_string(),
            "1.0.0".to_string(),
            "Analyze CSV files".to_string(),
            vec!["data".to_string(), "csv".to_string()],
            vec![],
            ResourceRequirements {
                timeout_secs: 30,
                memory_mb: 512,
                dependencies: vec!["python>=3.9".to_string()],
            },
            Some(SkillPricing {
                free_tier_calls_per_day: Some(100),
                cost_per_call_yocto: Some(1_000_000_000_000),
            }),
            "QmHash".to_string(),
            "checksum123".to_string(),
        );

        assert!(result);

        // Get skill
        let skill = contract.get_skill("csv-analyzer@1.0.0".to_string()).unwrap();
        assert_eq!(skill.name, "csv-analyzer");
        assert_eq!(skill.rating, 5.0);
    }

    #[test]
    fn test_discover_skills_by_tag() {
        let mut context = get_context(false);
        testing_env!(context.build());

        let mut contract = AgentRegistry::default();

        // Register agents and skills
        testing_env!(context.signer_account_id(accounts(1)).build());
        contract.register(
            "Agent 1".to_string(),
            vec![],
            None,
            "key1".to_string(),
            "Agent 1".to_string(),
        );
        contract.register_skill(
            "analyzer".to_string(),
            "1.0.0".to_string(),
            "Data analyzer".to_string(),
            vec!["data".to_string(), "analysis".to_string()],
            vec![],
            ResourceRequirements {
                timeout_secs: 30,
                memory_mb: 512,
                dependencies: vec![],
            },
            None,
            "Qm1".to_string(),
            "check1".to_string(),
        );

        // Discover by tag
        testing_env!(context.is_view(true).build());
        let results = contract.discover_skills("data".to_string(), None, Some(10));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "analyzer");
    }

    #[test]
    fn test_rate_skill() {
        let mut context = get_context(false);
        testing_env!(context.build());

        let mut contract = AgentRegistry::default();

        // Register two agents
        testing_env!(context.signer_account_id(accounts(1)).build());
        contract.register(
            "Agent 1".to_string(),
            vec![],
            None,
            "key1".to_string(),
            "Agent 1".to_string(),
        );

        testing_env!(context.signer_account_id(accounts(2)).build());
        contract.register(
            "Agent 2".to_string(),
            vec![],
            None,
            "key2".to_string(),
            "Agent 2".to_string(),
        );

        // Agent 1 registers a skill
        testing_env!(context.signer_account_id(accounts(1)).build());
        contract.register_skill(
            "skill1".to_string(),
            "1.0.0".to_string(),
            "Test skill".to_string(),
            vec![],
            vec![],
            ResourceRequirements {
                timeout_secs: 30,
                memory_mb: 512,
                dependencies: vec![],
            },
            None,
            "Qm1".to_string(),
            "check1".to_string(),
        );

        // Agent 2 rates Agent 1's skill
        testing_env!(context.signer_account_id(accounts(2)).build());
        let result = contract.rate_skill("skill1@1.0.0".to_string(), 4);
        assert!(result);

        // Check rating
        testing_env!(context.is_view(true).build());
        let skill = contract.get_skill("skill1@1.0.0".to_string()).unwrap();
        assert_eq!(skill.rating, 4.0);
        assert_eq!(skill.rating_count, 1);
    }
}
