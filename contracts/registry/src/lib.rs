use near_sdk::{env, near, AccountId, BorshStorageKey, require, collections::UnorderedMap, collections::UnorderedSet};
use borsh::{BorshDeserialize, BorshSerialize};
use borsh::BorshSchema;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod trust;
mod registration;
pub use trust::{TrustConfig, TrustLevel, Endorsement};
pub use registration::AgentRegistration;

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
    Endorsements,
    EndorsementsByAgent,
    AgentRegistrations,
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
    /// Web of Trust: endorsements received by agent
    endorsements: UnorderedMap<AccountId, Vec<Endorsement>>,
    /// Index: endorsements given by agent
    endorsements_by_agent: UnorderedMap<AccountId, Vec<(AccountId, String)>>,
    /// Agent registrations (Variant C)
    agent_registrations: UnorderedMap<AccountId, AgentRegistration>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self {
            agents: UnorderedMap::new(StorageKey::Agents),
            online_agents: UnorderedSet::new(StorageKey::OnlineAgents),
            skills: UnorderedMap::new(StorageKey::Skills),
            skills_by_agent: UnorderedMap::new(StorageKey::SkillsByAgent),
            skills_by_tag: UnorderedMap::new(StorageKey::SkillsByTag),
            endorsements: UnorderedMap::new(StorageKey::Endorsements),
            endorsements_by_agent: UnorderedMap::new(StorageKey::EndorsementsByAgent),
            agent_registrations: UnorderedMap::new(StorageKey::AgentRegistrations),
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

    // ==================== WEB OF TRUST FUNCTIONS ====================

    /// Endorse an agent for a specific capability
    /// This creates a trust relationship in the Web of Trust
    pub fn endorse_agent(
        &mut self,
        endorsed: AccountId,
        capability: String,
        trust_level: TrustLevel,
    ) -> bool {
        let endorser = env::signer_account_id();
        require!(endorser != endorsed, "Cannot endorse yourself");
        require!(self.agents.get(&endorsed).is_some(), "Agent not found");
        require!(self.agents.get(&endorser).is_some(), "Only registered agents can endorse");

        let endorsement = Endorsement {
            endorser: endorser.to_string(),
            endorsed: endorsed.to_string(),
            capability: capability.clone(),
            trust_level,
            timestamp: env::block_timestamp(),
            revoked: false,
        };

        // Add to endorsements received
        let mut received = self.endorsements.get(&endorsed).unwrap_or_default();
        received.push(endorsement);
        self.endorsements.insert(&endorsed, &received);

        // Add to endorsements given index
        let mut given = self.endorsements_by_agent.get(&endorser).unwrap_or_default();
        given.push((endorsed.clone(), capability.clone()));
        self.endorsements_by_agent.insert(&endorser, &given);

        near_sdk::log!("Endorsed {} for {} by {}", endorsed, capability, endorser);
        true
    }

    /// Revoke an endorsement
    pub fn revoke_endorsement(
        &mut self,
        endorsed: AccountId,
        capability: String,
    ) -> bool {
        let endorser = env::signer_account_id();
        
        if let Some(mut endorsements) = self.endorsements.get(&endorsed) {
            for e in endorsements.iter_mut() {
                if e.endorser == endorser.to_string() && e.capability == capability && !e.revoked {
                    e.revoked = true;
                    self.endorsements.insert(&endorsed, &endorsements);
                    near_sdk::log!("Revoked endorsement for {}", endorsed);
                    return true;
                }
            }
        }
        false
    }

    /// Get all endorsements for an agent
    pub fn get_endorsements(&self, agent_id: AccountId) -> Vec<Endorsement> {
        self.endorsements.get(&agent_id).unwrap_or_default()
    }

    /// Get endorsements given by an agent
    pub fn get_endorsements_by(&self, endorser: AccountId) -> Vec<(AccountId, String)> {
        self.endorsements_by_agent.get(&endorser).unwrap_or_default()
    }

    /// Compute trust score using Web of Trust algorithm
    /// Takes into account:
    /// - Endorser's own reputation
    /// - Trust level (partial vs full)
    /// - Time decay
    /// - Transitive trust paths
    pub fn compute_trust_score(&self, agent_id: AccountId, capability: String) -> u32 {
        let endorsements = match self.endorsements.get(&agent_id) {
            Some(e) => e,
            None => return 50, // Default trust
        };

        let config = TrustConfig::default();
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for endorsement in endorsements.iter() {
            if endorsement.revoked || endorsement.capability != capability {
                continue;
            }

            // Get endorser's reputation
            let endorser_account: AccountId = match endorsement.endorser.parse() {
                Ok(id) => id,
                Err(_) => continue, // Skip invalid account IDs
            };
            let endorser_trust = match self.agents.get(&endorser_account) {
                Some(meta) => meta.reputation,
                None => continue, // Skip unknown endorsers
            };

            // Skip low-trust endorsers
            if endorser_trust < config.min_endorser_trust {
                continue;
            }

            // Time decay
            let age_ms = env::block_timestamp() - endorsement.timestamp;
            let age_days = age_ms / (1000 * 60 * 60 * 24);
            let decay_factor = if age_days > config.trust_decay_days {
                0.5
            } else {
                1.0
            };

            // Weight by endorser's trust and trust level
            let weight = (endorser_trust as f32 / 100.0) 
                * endorsement.trust_level.weight() 
                * decay_factor;

            weighted_sum += endorser_trust as f32 * weight;
            total_weight += weight;
        }

        if total_weight > 0.0 {
            (weighted_sum / total_weight).min(100.0) as u32
        } else {
            50 // Default for no endorsements
        }
    }

    /// Find trust path from one agent to another (for transitive trust)
    pub fn find_trust_path(
        &self,
        source: AccountId,
        target: AccountId,
        capability: String,
        max_depth: Option<u32>,
    ) -> Option<Vec<AccountId>> {
        if source == target {
            return Some(vec![source]);
        }

        let max_depth = max_depth.unwrap_or(3);
        let mut visited: Vec<AccountId> = Vec::new();
        let mut queue = vec![(source.clone(), vec![source.clone()], 0u32)];

        while let Some((current, path, depth)) = queue.pop() {
            if depth >= max_depth {
                continue;
            }

            if let Some(endorsements) = self.endorsements.get(&current) {
                for e in endorsements.iter() {
                    if e.revoked || e.capability != capability {
                        continue;
                    }

                    // Convert String to AccountId
                    let endorsed_account: AccountId = match e.endorsed.parse() {
                        Ok(id) => id,
                        Err(_) => continue, // Skip invalid account IDs
                    };

                    if endorsed_account == target {
                        let mut result = path.clone();
                        result.push(endorsed_account.clone());
                        return Some(result);
                    }

                    if !visited.contains(&endorsed_account) {
                        visited.push(endorsed_account.clone());
                        let mut new_path = path.clone();
                        new_path.push(endorsed_account.clone());
                        queue.push((endorsed_account.clone(), new_path, depth + 1));
                    }
                }
            }
        }

        None
    }

    /// Discover agents by capability with trust filtering
    /// Returns agents sorted by trust score for the capability
    pub fn discover_trusted(
        &self,
        capability: String,
        min_trust: Option<u32>,
        limit: Option<u32>,
    ) -> Vec<(AgentMetadata, u32)> {
        let min_trust = min_trust.unwrap_or(50);
        let limit = limit.unwrap_or(20) as usize;

        let mut results: Vec<(AgentMetadata, u32)> = self.agents
            .iter()
            .filter(|(_, meta)| meta.capabilities.contains(&capability))
            .map(|(_, meta)| {
                let trust = self.compute_trust_score(
                    meta.account_id.parse().unwrap(),
                    capability.clone()
                );
                (meta, trust)
            })
            .filter(|(_, trust)| *trust >= min_trust)
            .collect();

        // Sort by trust score descending
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.into_iter().take(limit).collect()
    }

    // ==================== VARIANT C: AGENT REGISTRATION ====================

    /// Register agent's P2P public key on-chain (Variant C)
    /// This proves ownership of NEAR account and enables certificate-based verification
    pub fn register_agent_key(&mut self, public_key: Vec<u8>) -> bool {
        let account_id = env::signer_account_id();
        
        let registration = AgentRegistration::new(public_key);
        self.agent_registrations.insert(&account_id, &registration);
        
        near_sdk::log!("Agent key registered for: {}", account_id);
        true
    }

    /// Verify agent's P2P public key is registered (on-chain check)
    pub fn verify_agent_key(&self, account_id: AccountId, public_key: Vec<u8>) -> bool {
        match self.agent_registrations.get(&account_id) {
            Some(reg) => reg.public_key == public_key && reg.is_valid(),
            None => false,
        }
    }

    /// Revoke agent registration (can only revoke own key)
    pub fn revoke_agent_key(&mut self) -> bool {
        let account_id = env::signer_account_id();
        
        match self.agent_registrations.remove(&account_id) {
            Some(_) => {
                near_sdk::log!("Agent key revoked for: {}", account_id);
                true
            }
            None => false,
        }
    }

    /// Get agent registration info
    pub fn get_agent_registration(&self, account_id: AccountId) -> Option<AgentRegistration> {
        self.agent_registrations.get(&account_id)
    }
}
