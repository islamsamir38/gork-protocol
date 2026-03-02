# Agent Skills Extension - Implementation Guide

Add this to your `contracts/registry/src/lib.rs` to support Agent Skills.

## Step 1: Extend Storage Keys

Replace your existing `StorageKey` enum with:

```rust
/// Storage keys for the contract
#[derive(BorshSerialize, BorshDeserialize, BorshStorageKey)]
pub enum StorageKey {
    Agents,
    AgentsKeys,
    OnlineAgents,
    OnlineAgentsKeys,
    Skills,           // NEW: Individual skills
    SkillsByAgent,    // NEW: Agent -> their skills
    SkillsByTag,      // NEW: Tag -> skills with that tag
}
```

## Step 2: Add New Structs

Add these after your `AgentMetadata` struct:

```rust
/// Skill manifest following Agent Skills format (agentskills.io)
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SkillManifest {
    /// Unique skill identifier (name@version)
    pub skill_id: String,

    /// Human-readable name
    pub name: String,

    /// Version string
    pub version: String,

    /// Skill author (NEAR account)
    pub author: AccountId,

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
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CapabilityDetail {
    pub name: String,
    pub description: String,
    pub input_schema: String,   // JSON Schema as string
    pub output_schema: String,  // JSON Schema as string
    pub examples: Vec<String>,  // Example inputs
}

/// Resource requirements
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ResourceRequirements {
    pub timeout_secs: u32,
    pub memory_mb: u32,
    pub dependencies: Vec<String>,
}

/// Pricing model
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SkillPricing {
    pub free_tier_calls_per_day: Option<u32>,
    pub cost_per_call_yocto: Option<U128>,
}
```

## Step 3: Extend Contract State

Replace your `AgentRegistry` struct with:

```rust
#[near(contract_state)]
pub struct AgentRegistry {
    /// All registered agents by account ID
    agents: UnorderedMap<AccountId, AgentMetadata>,

    /// Set of online agents for quick lookup
    online_agents: UnorderedSet<AccountId>,

    // NEW: Skills storage
    /// All skills by skill_id (name@version)
    skills: UnorderedMap<String, SkillManifest>,

    /// Index: agent -> list of their skill_ids
    skills_by_agent: UnorderedMap<AccountId, Vec<String>>,

    /// Index: tag -> list of skill_ids with that tag
    skills_by_tag: UnorderedMap<String, Vec<String>>,
}
```

## Step 4: Update Default Implementation

Replace your `Default` impl with:

```rust
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
```

## Step 5: Add Skill Methods

Add these methods to your `#[near] impl AgentRegistry` block (before the closing brace):

```rust
    // ============= NEW SKILLS METHODS =============

    /// Register a skill (Agent Skills format)
    ///
    /// # Arguments
    /// * `name` - Skill name (unique identifier base)
    /// * `version` - Skill version
    /// * `description` - Human-readable description
    /// * `tags` - List of tags for discovery
    /// * `capabilities` - Detailed capabilities with schemas
    /// * `requirements` - Resource requirements
    /// * `pricing` - Optional pricing model
    /// * `ipfs_hash` - IPFS hash of full skill package
    /// * `checksum` - SHA256 checksum for verification
    ///
    /// # Example
    /// ```
    /// register_skill(
    ///     "csv-analyzer",
    ///     "1.0.0",
    ///     "Analyze CSV files",
    ///     vec!["data".to_string(), "csv".to_string()],
    ///     vec![capability_detail],
    ///     resource_reqs,
    ///     Some(pricing),
    ///     "Qm...".to_string(),
    ///     "abc123...".to_string()
    /// )
    /// ```
    #[payable(deposit)]
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
                // Only author can update their skill
                require!(existing.author == author, "Only skill author can update");
                (existing.usage_count, existing.rating, existing.rating_count)
            }
            None => (0, 5.0, 0), // Default 5-star rating for new skills
        };

        let manifest = SkillManifest {
            skill_id: skill_id.clone(),
            name: name.clone(),
            version,
            author: author.clone(),
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
    ///
    /// # Arguments
    /// * `tag` - Tag to search for
    /// * `min_rating` - Minimum rating (1.0 - 5.0)
    /// * `limit` - Maximum results
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

    /// Search skills by name fragment
    pub fn search_skills(
        &self,
        query: String,
        limit: Option<u32>,
    ) -> Vec<SkillManifest> {
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
            .filter_map(|skill| self.agents.get(&skill.author))
            .collect()
    }

    /// Rate a skill (1-5 stars)
    ///
    /// # Arguments
    /// * `skill_id` - Skill to rate
    /// * `rating` - Rating from 1 to 5
    ///
    /// # Requirements
    /// - Caller must be a registered agent
    /// - Rating must be between 1 and 5
    pub fn rate_skill(&mut self, skill_id: String, rating: u32) -> bool {
        let caller = env::signer_account_id();

        // Caller must be registered
        require!(self.agents.get(&caller).is_some(), "Caller not registered");

        // Rating must be valid
        require!(rating >= 1 && rating <= 5, "Rating must be 1-5");

        if let Some(mut skill) = self.skills.get(&skill_id) {
            // Can't rate your own skill
            require!(skill.author != caller, "Cannot rate your own skill");

            // Update average rating
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

    /// Track skill usage (called when skill is executed)
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
    pub fn get_top_skills(&self, limit: Option<u32>) -> Vec<SkillManifest> {
        let limit = limit.unwrap_or(10) as usize;

        let mut skills: Vec<_> = self.skills.values().collect();
        skills.sort_by(|a, b| b.usage_count.cmp(&a.usage_count));
        skills.into_iter().take(limit).cloned().collect()
    }
}

/// Skill statistics (lighter than full manifest)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStats {
    pub skill_id: String,
    pub usage_count: u32,
    pub rating: f32,
    pub rating_count: u32,
    pub author: String,
}
```

## Step 6: Add Tests

Add these to your test module:

```rust
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
                cost_per_call_yocto: Some(1_000_000_000_000), // 0.001 NEAR
            }),
            "QmHash".to_string(),
            "checksum123".to_string(),
        );

        assert!(result);

        // Get skill
        let skill = contract.get_skill("csv-analyzer@1.0.0".to_string()).unwrap();
        assert_eq!(skill.name, "csv-analyzer");
        assert_eq!(skill.rating, 5.0); // Default rating
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
```

## What This Gives You

✅ **Backward Compatible** - All your existing agent methods still work
✅ **Skill Registration** - Register rich skill manifests
✅ **Tag Discovery** - Find skills by tags
✅ **Skill Ratings** - Rate skills 1-5 stars
✅ **Usage Tracking** - Track how many times skills are used
✅ **Agent Skills** - Get all skills from an agent
✅ **Pricing Support** - Optional pricing per skill
✅ **IPFS Integration** - Store full skill packages on IPFS

## Usage Example

```bash
# 1. Register as agent (existing)
gork-agent register

# 2. Publish skill (NEW)
gork-agent skills publish ./my-skill/
# This calls register_skill() on your contract

# 3. Discover skills (NEW)
gork-agent skills search --tag data-analysis
# This calls discover_skills() on your contract

# 4. Find agents with skill (NEW)
gork-agent discover find-agents --skill csv-analyzer
# This calls find_agents_with_skill() on your contract

# 5. Rate skill (NEW)
gork-agent marketplace rate csv-analyzer 5
# This calls rate_skill() on your contract
```

## Next Steps

1. Copy these code blocks into `contracts/registry/src/lib.rs`
2. Run `cargo test` to verify tests pass
3. Build with `cargo build --target wasm32-unknown-unknown --release`
4. Deploy to NEAR testnet
5. Integrate with CLI

Want me to help with any of these steps?
