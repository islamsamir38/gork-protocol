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
    Skills,
    SkillsByAgent,
    SkillsByTag,
}

/// Agent metadata stored on-chain
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, BorshSchema, JsonSchema,
)]
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

/// Skill manifest with content-addressable ID
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, JsonSchema)]
pub struct SkillManifest {
    /// Content-addressable ID: "name@version:sha256:{code_hash}"
    pub skill_id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub tags: Vec<String>,
    pub capabilities: Vec<CapabilityDetail>,
    pub requirements: ResourceRequirements,
    pub pricing: Option<SkillPricing>,
    pub ipfs_hash: String,
    /// SHA-256 hash of the SKILL.md file (Agent Skills format)
    pub skill_md_hash: String,
    pub usage_count: u32,
    pub rating: f32,
    pub rating_count: u32,
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
    pub cost_per_call_yocto: Option<String>,
}

/// Skill statistics
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillStats {
    pub skill_id: String,
    pub usage_count: u32,
    pub rating: f32,
    pub rating_count: u32,
    pub author: String,
    pub skill_md_hash: String,
}

/// Verification result
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VerificationResult {
    pub skill_id: String,
    pub valid: bool,
    pub skill_md_hash_matches: bool,
}

#[near(contract_state)]
pub struct AgentRegistry {
    agents: UnorderedMap<AccountId, AgentMetadata>,
    online_agents: UnorderedSet<AccountId>,
    skills: UnorderedMap<String, SkillManifest>,
    skills_by_agent: UnorderedMap<AccountId, Vec<String>>,
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
    // ==================== AGENT FUNCTIONS ====================

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

        let (reputation, rating_count) = match self.agents.get(&account_id) {
            Some(metadata) => (metadata.reputation, metadata.rating_count),
            None => (50, 0),
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

    pub fn heartbeat(&mut self) -> bool {
        let account_id = env::signer_account_id();
        require!(self.agents.get(&account_id).is_some(), "Agent not registered");
        
        let mut metadata = self.agents.get(&account_id).unwrap();
        metadata.last_seen = env::block_timestamp();
        metadata.online = true;
        self.agents.insert(&account_id, &metadata);
        self.online_agents.insert(&account_id);
        near_sdk::log!("Heartbeat received from: {}", account_id);
        true
    }

    pub fn set_offline(&mut self) -> bool {
        let account_id = env::signer_account_id();
        require!(self.agents.get(&account_id).is_some(), "Agent not registered");
        
        let mut metadata = self.agents.get(&account_id).unwrap();
        metadata.online = false;
        self.agents.insert(&account_id, &metadata);
        self.online_agents.remove(&account_id);
        true
    }

    pub fn get_agent(&self, account_id: AccountId) -> Option<AgentMetadata> {
        self.agents.get(&account_id)
    }

    pub fn discover(
        &self,
        capability: String,
        online_only: bool,
        limit: Option<u32>,
    ) -> Vec<AgentMetadata> {
        let limit = limit.unwrap_or(20) as usize;
        let mut results = Vec::new();

        for (_, metadata) in self.agents.iter() {
            if metadata.capabilities.contains(&capability) {
                if !online_only || metadata.online {
                    results.push(metadata);
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }
        results
    }

    pub fn get_all_agents(&self, from_index: Option<u64>, limit: Option<u64>) -> Vec<AgentMetadata> {
        let from = from_index.unwrap_or(0) as usize;
        let limit = limit.unwrap_or(50) as usize;
        
        self.agents.iter()
            .skip(from)
            .take(limit)
            .map(|(_, v)| v)
            .collect()
    }

    pub fn get_online_count(&self) -> u32 {
        self.online_agents.len() as u32
    }

    pub fn get_total_count(&self) -> u32 {
        self.agents.len() as u32
    }

    pub fn rate_agent(&mut self, agent_id: AccountId, score: u32) -> bool {
        let rater = env::signer_account_id();
        require!(rater != agent_id, "Cannot rate yourself");
        require!(score >= 1 && score <= 100, "Score must be 1-100");
        require!(self.agents.get(&agent_id).is_some(), "Agent not found");

        let mut metadata = self.agents.get(&agent_id).unwrap();
        let total = (metadata.reputation as u64 * metadata.rating_count as u64 + score as u64);
        metadata.rating_count += 1;
        metadata.reputation = (total / metadata.rating_count as u64) as u32;
        self.agents.insert(&agent_id, &metadata);
        true
    }

    pub fn unregister(&mut self) -> bool {
        let account_id = env::signer_account_id();
        require!(self.agents.get(&account_id).is_some(), "Agent not registered");
        
        self.agents.remove(&account_id);
        self.online_agents.remove(&account_id);
        true
    }

    pub fn update_capabilities(&mut self, capabilities: Vec<String>) -> bool {
        let account_id = env::signer_account_id();
        require!(self.agents.get(&account_id).is_some(), "Agent not registered");
        
        let mut metadata = self.agents.get(&account_id).unwrap();
        metadata.capabilities = capabilities;
        self.agents.insert(&account_id, &metadata);
        true
    }

    pub fn update_endpoint(&mut self, endpoint: Option<String>) -> bool {
        let account_id = env::signer_account_id();
        require!(self.agents.get(&account_id).is_some(), "Agent not registered");
        
        let mut metadata = self.agents.get(&account_id).unwrap();
        metadata.endpoint = endpoint;
        self.agents.insert(&account_id, &metadata);
        true
    }

    // ==================== SKILL FUNCTIONS (CONTENT-ADDRESSABLE) ====================

    /// Register a skill with content-addressable ID
    /// The skill_id will be: "name@version:sha256:{skill_md_hash}"
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
        skill_md_hash: String, // SHA-256 of SKILL.md file (provided by caller)
    ) -> bool {
        let author = env::signer_account_id();
        
        // Create content-addressable skill_id from SKILL.md hash
        let skill_id = format!("{}@{}:sha256:{}", name, version, skill_md_hash);
        
        let skill = SkillManifest {
            skill_id: skill_id.clone(),
            name,
            version,
            author: author.to_string(),
            description,
            tags: tags.clone(),
            capabilities,
            requirements,
            pricing,
            ipfs_hash,
            skill_md_hash,
            usage_count: 0,
            rating: 5.0,
            rating_count: 0,
            created_at: env::block_timestamp(),
        };

        // Store skill
        self.skills.insert(&skill_id, &skill);

        // Update agent's skill list
        let mut agent_skills = self.skills_by_agent.get(&author).unwrap_or_default();
        agent_skills.push(skill_id.clone());
        self.skills_by_agent.insert(&author, &agent_skills);

        // Update tag indexes
        for tag in tags {
            let mut tag_skills = self.skills_by_tag.get(&tag).unwrap_or_default();
            tag_skills.push(skill_id.clone());
            self.skills_by_tag.insert(&tag, &tag_skills);
        }

        near_sdk::log!("Skill registered: {} by {}", skill_id, author);
        true
    }

    /// Verify a skill's SKILL.md hash matches what was registered
    pub fn verify_skill(&self, skill_id: String, skill_md_hash: String) -> VerificationResult {
        match self.skills.get(&skill_id) {
            Some(skill) => {
                let matches = skill.skill_md_hash == skill_md_hash;
                VerificationResult {
                    skill_id: skill_id.clone(),
                    valid: matches,
                    skill_md_hash_matches: matches,
                }
            }
            None => VerificationResult {
                skill_id,
                valid: false,
                skill_md_hash_matches: false,
            }
        }
    }

    /// Get skill by content-addressable ID
    pub fn get_skill(&self, skill_id: String) -> Option<SkillManifest> {
        self.skills.get(&skill_id)
    }

    /// Discover skills by tag
    pub fn discover_skills(&self, tag: String, limit: Option<u32>) -> Vec<SkillManifest> {
        let limit = limit.unwrap_or(20) as usize;
        match self.skills_by_tag.get(&tag) {
            Some(skill_ids) => skill_ids
                .iter()
                .take(limit)
                .filter_map(|id| self.skills.get(id))
                .collect(),
            None => vec![],
        }
    }

    /// Search skills by name/description
    pub fn search_skills(&self, query: String, limit: Option<u32>) -> Vec<SkillManifest> {
        let limit = limit.unwrap_or(20) as usize;
        let query_lower = query.to_lowercase();
        
        self.skills
            .iter()
            .filter(|(_, skill)| {
                skill.name.to_lowercase().contains(&query_lower)
                    || skill.description.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .map(|(_, v)| v)
            .collect()
    }

    /// Get all skills by an agent
    pub fn get_agent_skills(&self, agent_id: AccountId) -> Vec<SkillManifest> {
        match self.skills_by_agent.get(&agent_id) {
            Some(skill_ids) => skill_ids
                .iter()
                .filter_map(|id| self.skills.get(id))
                .collect(),
            None => vec![],
        }
    }

    /// Find agents with a specific skill (by skill name, not ID)
    pub fn find_agents_with_skill(&self, skill_name: String) -> Vec<AgentMetadata> {
        let mut results = Vec::new();
        
        for (_, skill) in self.skills.iter() {
            if skill.name == skill_name {
                if let Ok(author_id) = AccountId::try_from(skill.author.clone()) {
                    if let Some(metadata) = self.agents.get(&author_id) {
                        results.push(metadata);
                    }
                }
            }
        }
        results
    }

    /// Rate a skill (can't rate own skill)
    pub fn rate_skill(&mut self, skill_id: String, rating: u32) -> bool {
        let rater = env::signer_account_id();
        require!(rating >= 1 && rating <= 5, "Rating must be 1-5");
        
        match self.skills.get(&skill_id) {
            Some(mut skill) => {
                require!(skill.author != rater.to_string(), "Cannot rate your own skill");
                
                let total = (skill.rating as f32 * skill.rating_count as f32 + rating as f32);
                skill.rating_count += 1;
                skill.rating = total / skill.rating_count as f32;
                self.skills.insert(&skill_id, &skill);
                true
            }
            None => env::panic_str("Skill not found"),
        }
    }

    /// Track skill usage
    pub fn track_skill_usage(&mut self, skill_id: String) -> bool {
        match self.skills.get(&skill_id) {
            Some(mut skill) => {
                skill.usage_count += 1;
                self.skills.insert(&skill_id, &skill);
                true
            }
            None => env::panic_str("Skill not found"),
        }
    }

    /// Get skill statistics (lighter than full manifest)
    pub fn get_skill_stats(&self, skill_id: String) -> Option<SkillStats> {
        self.skills.get(&skill_id).map(|skill| SkillStats {
            skill_id: skill.skill_id,
            usage_count: skill.usage_count,
            rating: skill.rating,
            rating_count: skill.rating_count,
            author: skill.author,
            skill_md_hash: skill.skill_md_hash,
        })
    }

    /// Get top-rated skills
    pub fn get_top_skills(&self, limit: Option<u32>) -> Vec<SkillStats> {
        let limit = limit.unwrap_or(10) as usize;
        
        let mut skills: Vec<SkillStats> = self
            .skills
            .iter()
            .map(|(_, skill)| SkillStats {
                skill_id: skill.skill_id,
                usage_count: skill.usage_count,
                rating: skill.rating,
                rating_count: skill.rating_count,
                author: skill.author,
                skill_md_hash: skill.skill_md_hash,
            })
            .collect();

        skills.sort_by(|a, b| {
            b.rating.partial_cmp(&a.rating)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        skills.into_iter().take(limit).collect()
    }
}
