# Agent Skills Implementation Plan

## What to Add

### 1. Skills Module

```rust
// src/skills/mod.rs
pub mod manifest;
pub mod discovery;
pub mod executor;
pub mod registry;
pub mod sandbox;

pub use manifest::{SkillManifest, Capability};
pub use discovery::SkillDiscovery;
pub use executor::SkillExecutor;
pub use registry::SkillRegistry;

use serde::{Deserialize, Serialize};

/// A skill package following agentskills.io format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPackage {
    pub manifest: SkillManifest,
    pub instructions: String,
    pub resources: Vec<Resource>,
    pub tests: Option<Vec<Test>>,
}

/// Skill manifest (skill.yaml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: AccountId,
    pub license: String,
    pub tags: Vec<String>,
    pub capabilities: Vec<Capability>,
    pub requirements: SkillRequirements,
    pub pricing: Option<SkillPricing>,
}

/// What a skill can do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub description: String,
    pub input: JsonSchema,
    pub output: JsonSchema,
    pub examples: Vec<Example>,
}

/// Pricing model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPricing {
    pub free_tier: Option<FreeTier>,
    pub paid: Option<PaidTier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeTier {
    pub calls_per_day: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaidTier {
    pub cost_per_call: U128,  // yoctoNEAR
}
```

### 2. CLI Commands

```rust
// src/main.rs - Add new commands

#[derive(Subcommand)]
enum SkillsCommand {
    /// List available skills
    List {
        #[arg(long)]
        tags: Option<String>,

        #[arg(long)]
        author: Option<String>,
    },

    /// Search for skills
    Search {
        query: String,

        #[arg(long)]
        tag: Option<String>,

        #[arg(long)]
        capability: Option<String>,
    },

    /// Inspect a skill
    Inspect {
        skill_name: String,
    },

    /// Publish a skill
    Publish {
        skill_path: PathBuf,

        #[arg(long)]
        price: Option<String>,

        #[arg(long)]
        free_tier: Option<u32>,
    },

    /// Test a skill locally
    Test {
        skill_path: PathBuf,

        #[arg(long)]
        capability: Option<String>,
    },

    /// Find agents with a skill
    FindAgents {
        skill_name: String,
    },
}

#[derive(Subcommand)]
enum ExecuteCommand {
    /// Execute a skill on another agent
    Use {
        #[arg(long)]
        agent: AccountId,

        #[arg(long)]
        skill: String,

        #[arg(long)]
        capability: String,

        #[arg(long)]
        input: String,

        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Chain multiple skills
    Chain {
        #[arg(long)]
        workflow: PathBuf,
    },
}
```

### 3. Skill Discovery (P2P)

```rust
// src/skills/discovery.rs
use libp2p::{gossipsub, kad, Swarm};
use tokio::sync::mpsc;

pub struct SkillDiscovery {
    swarm: Swarm<GorkBehaviour>,
    skill_topic: gossipsub::IdentTopic,
}

impl SkillDiscovery {
    pub async fn advertise(&mut self, skill: &SkillManifest) -> Result<()> {
        let ad = SkillAdvertisement {
            skill_name: skill.name.clone(),
            version: skill.version.clone(),
            author: skill.author.clone(),
            capabilities: skill.capabilities.iter()
                .map(|c| c.name.clone())
                .collect(),
            pricing: skill.pricing.clone(),
            peer_id: self.swarm.local_peer_id(),
        };

        // Publish to gossipsub
        let message = serde_json::to_vec(&ad)?;
        self.swarm.behaviour_mut()
            .publish(&self.skill_topic, message)?;

        Ok(())
    }

    pub async fn discover(&mut self, query: SkillQuery) -> Vec<SkillAdvertisement> {
        // Query Kademlia DHT
        // Subscribe to gossipsub for real-time updates
        // Return matching skills
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SkillAdvertisement {
    pub skill_name: String,
    pub version: String,
    pub author: AccountId,
    pub capabilities: Vec<String>,
    pub pricing: Option<SkillPricing>,
    pub peer_id: PeerId,
}

#[derive(Debug, Clone)]
pub struct SkillQuery {
    pub skill_name: Option<String>,
    pub tags: Option<Vec<String>>,
    pub capabilities: Option<Vec<String>>,
    pub min_rating: Option<f32>,
}
```

### 4. Skill Execution

```rust
// src/skills/executor.rs
use std::path::PathBuf;
use std::process::Command;

pub struct SkillExecutor {
    workspace: PathBuf,
}

impl SkillExecutor {
    pub async fn execute(
        &self,
        skill: &SkillPackage,
        capability: &str,
        input: Value,
    ) -> Result<Value> {
        // Find capability in manifest
        let cap = skill.manifest.capabilities.iter()
            .find(|c| c.name == capability)
            .ok_or_else(|| anyhow!("Capability not found"))?;

        // Validate input against schema
        self.validate_input(&cap.input, &input)?;

        // Execute in sandbox
        let output = self.run_sandboxed(skill, capability, &input).await?;

        // Validate output
        self.validate_output(&cap.output, &output)?;

        Ok(output)
    }

    async fn run_sandboxed(
        &self,
        skill: &SkillPackage,
        capability: &str,
        input: &Value,
    ) -> Result<Value> {
        // Create temporary workspace
        let work_dir = self.workspace.join(&skill.manifest.name);
        fs::create_dir_all(&work_dir)?;

        // Write input file
        let input_file = work_dir.join("input.json");
        fs::write(&input_file, serde_json::to_string_pretty(input)?)?;

        // Execute skill (example: Docker sandbox)
        let output = Command::new("docker")
            .args([
                "run", "--rm",
                "-v", &format!("{}:/workspace", work_dir.display()),
                "-w", "/workspace",
                "gork-sandbox:latest",
                &format!("{}::{}", skill.manifest.name, capability),
                "/workspace/input.json"
            ])
            .output()
            .await?;

        if !output.status.success() {
            bail!("Skill execution failed: {}",
                String::from_utf8_lossy(&output.stderr));
        }

        let result: Value = serde_json::from_slice(&output.stdout)?;
        Ok(result)
    }
}
```

### 5. NEAR Registry Contract

```rust
// contracts/skill-registry/src/lib.rs
use near_sdk::{AccountId, BorshStorageKey, PanicOnDefault, Promise};

#[near_sdk::near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct SkillRegistry {
    skills: near_sdk::collections::LookupMap<String, SkillMetadata>,
    agent_skills: near_sdk::collections::LookupMap<AccountId, Vec<String>>,
    ratings: near_sdk::collections::LookupMap<String, Vec<Rating>>,
}

#[derive(BorshDeserialize, BorshSerialize, serde::Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SkillMetadata {
    pub id: String,
    pub name: String,
    pub author: AccountId,
    pub ipfs_hash: String,
    pub checksum: String,
    pub version: String,
    pub tags: Vec<String>,
    pub capabilities: Vec<String>,
    pub price: Option<U128>,
    pub created_at: U64,
}

#[near_sdk::near_bindgen]
impl SkillRegistry {
    #[payable(deposit)]
    pub fn register_skill(&mut self, metadata: SkillMetadata) -> Promise {
        // Validate metadata
        // Verify author signature
        // Store skill
        // Emit event
        Promise::new(env::predecessor_account_id())
    }

    pub fn get_skill(&self, id: String) -> Option<SkillMetadata> {
        self.skills.get(&id)
    }

    pub fn find_by_tags(&self, tags: Vec<String>) -> Vec<SkillMetadata> {
        self.skills.values()
            .filter(|s| s.tags.iter().any(|t| tags.contains(t)))
            .collect()
    }

    pub fn find_agents_with_skill(&self, skill_id: String) -> Vec<AccountId> {
        self.agent_skills.iter()
            .filter(|(_, skills)| skills.contains(&skill_id))
            .map(|(agent, _)| agent)
            .collect()
    }

    pub fn rate_skill(
        &mut self,
        skill_id: String,
        rating: u8,
        comment: String,
    ) {
        // Add rating (1-5)
        // Update average
    }

    pub fn get_skill_rating(&self, skill_id: String) -> Option<f64> {
        // Calculate average rating
    }
}
```

## Usage Examples

### Publish a Skill

```bash
# 1. Create skill following Agent Skills format
mkdir my-skill
cd my-skill

cat > skill.yaml <<EOF
name: sentiment-analyzer
version: 1.0.0
description: Analyze sentiment of text using ML
author: myaccount.near
tags: [nlp, sentiment, ml]
capabilities:
  - name: analyze-sentiment
    description: Analyze text sentiment
    input:
      type: object
      properties:
        text:
          type: string
    output:
      type: object
      properties:
        sentiment:
          type: string
          enum: [positive, negative, neutral]
        confidence:
          type: number
EOF

cat > instructions.md <<EOF
# Sentiment Analysis

Analyzes text sentiment using ML model.
EOF

# 2. Validate skill
gork-agent skills validate ./

# 3. Test locally
gork-agent skills test ./ \
  --capability analyze-sentiment \
  --input '{"text": "I love this!"}'

# 4. Publish to network
gork-agent skills publish ./ \
  --price "0.001 NEAR" \
  --free-tier 100
```

### Discover and Use Skills

```bash
# Find agents with sentiment analysis
gork-agent skills find-agents --skill sentiment-analyzer

# Search for NLP skills
gork-agent skills search --tag nlp

# Use the skill
gork-agent execute use \
  --agent analyst.near \
  --skill sentiment-analyzer \
  --capability analyze-sentiment \
  --input '{"text": "This is amazing!"}' \
  --output result.json

# Chain multiple skills
cat > workflow.yaml <<EOF
steps:
  - name: analyze
    agent: analyst.near
    skill: sentiment-analyzer
    capability: analyze-sentiment
    input:
      text: "Customer feedback here"

  - name: categorize
    agent: organizer.near
    skill: ticket-categorizer
    capability: categorize
    input_from: analyze
EOF

gork-agent execute chain --workflow workflow.yaml
```

## New Dependencies

```toml
[dependencies]
# Skill format validation
schemars = "0.8"
jsonschema = "0.17"

# Sandbox
docker = "0.1"

# IPFS for skill storage
rust-ipfs = "0.5"

# Additional crypto
ed25519-dalek = "2.0"
```

## Directory Structure

```
src/
├── skills/              # NEW: Skills module
│   ├── mod.rs
│   ├── manifest.rs      # Skill format
│   ├── discovery.rs     # P2P discovery
│   ├── executor.rs      # Skill execution
│   ├── registry.rs      # NEAR registry client
│   └── sandbox.rs       # Sandboxing
├── cli/                 # NEW: CLI commands
│   ├── mod.rs
│   ├── skills.rs        # Skills commands
│   ├── execute.rs       # Execution commands
│   └── marketplace.rs   # Marketplace commands
├── network/             # Existing P2P
├── crypto/              # Existing crypto
└── storage/             # Existing storage
```

## What You Get

✅ **Decentralized Skill Marketplace**
- No central server
- P2P discovery
- Direct agent-to-agent execution

✅ **Agent Skills Compatible**
- Follows open standard
- Portable skills
- Interoperable

✅ **Verified Identity**
- NEAR account verification
- Reputation system
- Trusted execution

✅ **Monetization**
- Charge for skill usage
- Free tiers
- Micropayments via NEAR

✅ **Composability**
- Chain multiple skills
- Multi-agent workflows
- Complex task automation

This would make Gork a **decentralized Agent Skills marketplace** where:
- Agents discover each other's capabilities via P2P
- Skills are verified on NEAR blockchain
- Agents can charge for using their skills
- Reputation ensures quality
- Skills work across different agent platforms
