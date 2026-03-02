# Extending Your Existing Registry for Agent Skills

Great news! You already have a solid foundation. We just need to **extend** your `AgentRegistry` contract to support Agent Skills format.

## What You Already Have ✅

Your `contracts/registry/` already implements:
- ✅ Agent registration with metadata
- ✅ Capabilities (simple strings)
- ✅ Reputation/rating system
- ✅ Online status tracking
- ✅ Discovery by capability

## What to Add for Agent Skills

### Option 1: Minimal Extension (Recommended)

Just add **skill manifests** as structured capabilities. Extend your existing contract:

```rust
// contracts/registry/src/lib.rs

/// NEW: Full skill manifest (Agent Skills format)
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct SkillManifest {
    /// Skill name (unique)
    pub name: String,

    /// Version
    pub version: String,

    /// Skill author (must match signer)
    pub author: AccountId,

    /// Human-readable description
    pub description: String,

    /// Tags for discovery
    pub tags: Vec<String>,

    /// Detailed capabilities with schemas
    pub capabilities: Vec<CapabilityDetail>,

    /// Resource requirements
    pub requirements: ResourceRequirements,

    /// Pricing (optional)
    pub pricing: Option<SkillPricing>,

    /// IPFS hash for full skill package
    pub ipfs_hash: String,

    /// Checksum for verification
    pub checksum: String,

    /// Usage statistics
    pub usage_count: u32,

    /// Average rating (1-5)
    pub rating: f32,

    /// Total ratings
    pub rating_count: u32,
}

/// NEW: Detailed capability with JSON schemas
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CapabilityDetail {
    pub name: String,
    pub description: String,
    pub input_schema: String,  // JSON Schema string
    pub output_schema: String, // JSON Schema string
    pub examples: Vec<String>, // Example inputs
}

/// NEW: Resource requirements
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ResourceRequirements {
    pub timeout_secs: u32,
    pub memory_mb: u32,
    pub dependencies: Vec<String>,
}

/// NEW: Pricing model
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SkillPricing {
    pub free_tier_calls_per_day: Option<u32>,
    pub cost_per_call_yocto: Option<U128>,
}

// Extend your existing storage keys
#[derive(BorshSerialize, BorshDeserialize, BorshStorageKey)]
pub enum StorageKey {
    Agents,
    AgentsKeys,
    OnlineAgents,
    OnlineAgentsKeys,
    Skills,           // NEW
    SkillsByAgent,    // NEW
    SkillsByTag,      // NEW
}

// Extend your contract
#[near(contract_state)]
pub struct AgentRegistry {
    agents: UnorderedMap<AccountId, AgentMetadata>,
    online_agents: UnorderedSet<AccountId>,

    // NEW: Skill storage
    skills: UnorderedMap<String, SkillManifest>,          // skill_id -> manifest
    skills_by_agent: UnorderedMap<AccountId, Vec<String>>, // agent -> skill_ids
    skills_by_tag: UnorderedMap<String, Vec<String>>,      // tag -> skill_ids
}
```

### Add These Methods to Your Contract

```rust
#[near]
impl AgentRegistry {
    /// NEW: Register a skill (Agent Skills format)
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

        // Create skill ID
        let skill_id = format!("{}@{}", name, version);

        // Check if skill exists
        let (usage_count, rating, rating_count) = match self.skills.get(&skill_id) {
            Some(existing) => (existing.usage_count, existing.rating, existing.rating_count),
            None => (0, 5.0, 0), // Default rating
        };

        let manifest = SkillManifest {
            name,
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
        };

        // Store skill
        self.skills.insert(&skill_id, &manifest);

        // Index by agent
        let mut agent_skills = self.skills_by_agent.get(&author).unwrap_or_default();
        agent_skills.push(skill_id.clone());
        self.skills_by_agent.insert(&author, &agent_skills);

        // Index by tags
        for tag in tags {
            let mut tag_skills = self.skills_by_tag.get(&tag).unwrap_or_default();
            tag_skills.push(skill_id.clone());
            self.skills_by_tag.insert(&tag, &tag_skills);
        }

        near_sdk::log!("Skill registered: {} by {}", skill_id, author);
        true
    }

    /// NEW: Get skill by ID
    pub fn get_skill(&self, skill_id: String) -> Option<SkillManifest> {
        self.skills.get(&skill_id)
    }

    /// NEW: Discover skills by tag
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

    /// NEW: Find agents with a specific skill
    pub fn find_agents_with_skill(&self, skill_name: String) -> Vec<AgentMetadata> {
        self.skills.iter()
            .filter(|(_, skill)| skill.name == skill_name)
            .filter_map(|(_, skill)| self.agents.get(&skill.author))
            .collect()
    }

    /// NEW: Rate a skill
    pub fn rate_skill(&mut self, skill_id: String, rating: u32) -> bool {
        let caller = env::signer_account_id();

        // Caller must be registered
        require!(self.agents.get(&caller).is_some(), "Caller not registered");

        // Rating must be 1-5
        require!(rating >= 1 && rating <= 5, "Rating must be 1-5");

        if let Some(mut skill) = self.skills.get(&skill_id) {
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

    /// NEW: Track skill usage
    pub fn track_skill_usage(&mut self, skill_id: String) -> bool {
        if let Some(mut skill) = self.skills.get(&skill_id) {
            skill.usage_count += 1;
            self.skills.insert(&skill_id, &skill);
            true
        } else {
            false
        }
    }

    /// NEW: Get skills by agent
    pub fn get_agent_skills(&self, agent_id: AccountId) -> Vec<SkillManifest> {
        self.skills_by_agent.get(&agent_id)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|skill_id| self.skills.get(&skill_id))
            .collect()
    }

    /// UPDATED: Extend discovery to include skills
    pub fn discover_enhanced(
        &self,
        query: String,
        search_type: String, // "capability" | "skill" | "tag"
        limit: Option<u32>,
    ) -> Vec<DiscoverResult> {
        // Returns both agents and skills
    }
}

/// NEW: Combined discovery result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverResult {
    pub result_type: String, // "agent" | "skill"
    pub agent_id: Option<AccountId>,
    pub skill_id: Option<String>,
    pub name: String,
    pub description: String,
    pub rating: f32,
}
```

## CLI Integration

Your existing CLI just needs new commands:

```bash
# Register skill (uploads to IPFS, registers on NEAR)
gork-agent skills publish ./my-skill/

# Discover skills
gork-agent skills search --tag data-analysis

# Find agents with skill
gork-agent discover find-agents --skill csv-analyzer

# Execute skill (uses existing P2P!)
gork-agent execute use \
  --agent alice.near \
  --skill csv-analyzer \
  --capability analyze
```

## Migration Path

**Phase 1: Add Skills to Existing Contract** (1 week)

```rust
// Just add these storage keys to your existing contract
skills: UnorderedMap<String, SkillManifest>,
skills_by_agent: UnorderedMap<AccountId, Vec<String>>,
skills_by_tag: UnorderedMap<String, Vec<String>>,
```

**Phase 2: Skills Module in CLI** (1 week)

```rust
// src/skills/mod.rs
pub mod manifest;  // Agent Skills format
pub mod executor;  // Skill execution

// src/main.rs - extend CLI
Skills(Publish, Search, Inspect),
Execute(Use, Chain),
```

**Phase 3: P2P Skill Discovery** (1 week)

```rust
// Advertise skills via existing gossipsub
let skill_ad = SkillAdvertisement {
    skill_name: "csv-analyzer",
    version: "1.0.0",
    author: "alice.near",
};
gossipsub.publish("gork-skills", skill_ad);
```

## What You Get

**Keep what you have:**
- ✅ Agent registry
- ✅ Capability discovery
- ✅ Reputation system
- ✅ Online tracking

**Add:**
- 🆕 Rich skill manifests (Agent Skills format)
- 🆕 Skill discovery by tags
- 🆕 Skill usage tracking
- 🆕 Per-skill ratings
- 🆕 IPFS-based skill packages

**Minimal changes:**
- Extend existing contract (don't rewrite)
- Add skills module to CLI
- Use existing P2P for skill discovery
- Use existing reputation system

## Example Usage

### Register a skill

```bash
# 1. Create skill (Agent Skills format)
mkdir csv-analyzer
cat > skill.yaml <<EOF
name: csv-analyzer
version: 1.0.0
description: Analyze CSV files
tags: [data, csv, python]
EOF

# 2. Publish (uploads to IPFS + registers on NEAR)
gork-agent skills publish ./csv-analyzer

# 3. Skill now discoverable!
gork-agent skills search --tag csv
# Found: csv-analyzer by alice.near (4.8★)
```

### Use a skill

```bash
# 1. Find agents with skill
gork-agent discover find-agents --skill csv-analyzer

# 2. Execute via P2P (your existing network!)
gork-agent execute use \
  --agent alice.near \
  --skill csv-analyzer \
  --capability analyze \
  --input '{"file": "data.csv"}'
```

## Summary

**You're 70% there!** Your existing registry has:
- Agent discovery ✅
- Reputation ✅
- Online tracking ✅
- Capability system ✅

**Just need to add:**
- Rich skill manifests (Agent Skills format)
- Skill-specific ratings
- IPFS integration for skill packages
- Skills CLI commands

**Total work: 2-3 weeks** to extend what you already have!

Want me to start implementing the contract extensions?
