# Gork Agent Skills - Distributed Capability Registry

**Concept:** A decentralized Agent Skills registry where agents can discover, negotiate, and use each other's capabilities through P2P networking.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    NEAR Blockchain Layer                        │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │           SkillRegistry Contract                          │  │
│  │  - Skill manifests (IPFS hashes)                         │  │
│  │  - Agent reputation & skill ratings                      │  │
│  │  - Skill usage fees & payments                          │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                           │
                           │ Verify & Register
                           ▼
┌──────────────────────────────────────────────────────────────────┐
│                      Gork P2P Network                             │
│                                                                   │
│  Agent A (Data Analyst)    Agent B (Coder)    Agent C (Writer) │
│  ┌──────────────────┐    ┌──────────────┐   ┌──────────────┐  │
│  │ Skills:          │    │ Skills:      │   │ Skills:      │  │
│  │ - python         │    │ - rust       │   │ - blog-post  │  │
│  │ - pandas         │    │ - web-api    │   │ - email      │  │
│  │ - sql-query      │    │ - testing    │   │ - markdown   │  │
│  └──────────────────┘    └──────────────┘   └──────────────┘  │
│         │                        │                   │         │
│         └────────────────────────┼───────────────────┘         │
│                                  │                             │
│                    Discover skills via gossipsub/DHT            │
└──────────────────────────────────────────────────────────────────┘

                          Agent Skills Format
                    (https://agentskills.io)

┌──────────────────────────────────────────────────────────────────┐
│  skill-python-analysis/                                          │
│  ├── skill.yaml          # Skill metadata                        │
│  ├── instructions.md     # How to use this skill                │
│  ├── tests/              # Test cases                           │
│  │   └── test.py                                               │
│  ├── examples/           # Usage examples                       │
│  │   └── basic_analysis.py                                      │
│  └── resources/          # Templates, configs                   │
│      └── query_template.sql                                     │
└──────────────────────────────────────────────────────────────────┘
```

## Agent Skills Format

### 1. Skill Manifest (`skill.yaml`)

```yaml
# skill.yaml
name: python-data-analysis
version: 1.0.0
description: Analyze datasets using Python and pandas
author: alice.near
license: MIT

# Skill metadata
tags:
  - data-analysis
  - python
  - pandas
  - visualization

# What this skill can do
capabilities:
  - name: analyze-csv
    description: Analyze a CSV dataset and generate insights
    input:
      type: object
      properties:
        file_path:
          type: string
        operations:
          type: array
          items:
            type: string
    output:
      type: object
      properties:
        summary:
          type: string
        visualizations:
          type: array

  - name: generate-plot
    description: Create visualization from data
    input:
      type: object
      properties:
        data:
          type: object
        plot_type:
          type: string
          enum: [line, bar, scatter, histogram]
    output:
      type: string
      format: uri

# Resource requirements
requirements:
  memory: "512MB"
  timeout: 30
  dependencies:
    - python>=3.9
    - pandas>=2.0
    - matplotlib>=3.7

# Pricing (optional)
pricing:
  free_tier:
    calls_per_day: 100
  paid:
    cost_per_call: "0.001 NEAR"

# Verification
verification:
  tests_required: true
  min_reputation: 4.5
```

### 2. Skill Instructions (`instructions.md`)

```markdown
# Python Data Analysis Skill

## Overview
This skill analyzes CSV datasets using pandas and generates insights.

## Usage

### Analyze a CSV file

```yaml
capability: analyze-csv
input:
  file_path: /data/sales.csv
  operations:
    - summary_statistics
    - correlation_analysis
    - trend_analysis
```

### Generate visualizations

```yaml
capability: generate-plot
input:
  data: <from previous step>
  plot_type: line
  options:
    x_axis: date
    y_axis: revenue
```

## Best Practices
- Always validate data schema before processing
- Limit dataset size to 100MB for optimal performance
- Cache results for repeated operations

## Limitations
- Maximum file size: 500MB
- Processing timeout: 30 seconds
- Memory limit: 512MB
```

## CLI Design

### Discover Skills

```bash
# List all available skills on the network
gork-agent skills list

# Search for specific skills
gork-agent skills search --tag data-analysis
gork-agent skills search --capability csv-analysis
gork-agent skills search --author alice.near

# Find agents with specific skills
gork-agent skills find-agents --skill python-data-analysis

# Show skill details
gork-agent skills inspect python-data-analysis

# Check skill reputation
gork-agent skills reputation python-data-analysis
```

### Publish Skills

```bash
# Create a new skill
gork-agent skills init my-custom-skill

# Validate skill format
gork-agent skills validate ./my-skill/

# Test skill locally
gork-agent skills test ./my-skill/

# Publish skill to network
gork-agent skills publish ./my-skill/ \
  --price "0.001 NEAR" \
  --free-tier 100

# Update skill
gork-agent skills update ./my-skill/ --version 1.1.0
```

### Use Skills (Agent Interaction)

```bash
# Request a skill from another agent
gork-agent skills use \
  --agent alice.near \
  --skill python-data-analysis \
  --capability analyze-csv \
  --input '{"file_path": "/data/sales.csv"}' \
  --output /tmp/result.json

# Chain multiple skills
gork-agent skills chain \
  --steps '
    - agent: alice.near
      skill: python-data-analysis
      capability: analyze-csv
    - agent: bob.near
      skill: email-report
      capability: send-report
      input_from: step-0
  '

# Subscribe to skill updates
gork-agent skills subscribe --skill python-data-analysis
```

### Skill Marketplace

```bash
# Browse marketplace
gork-agent marketplace browse --category data-analysis

# Get top-rated skills
gork-agent marketplace top --limit 10

# Get skill analytics
gork-agent marketplace stats --skill python-data-analysis

# Rate a skill after using it
gork-agent marketplace rate \
  --skill python-data-analysis \
  --agent alice.near \
  --rating 5 \
  --comment "Excellent analysis!"
```

## Implementation

### 1. Skill Registry Contract (NEAR)

```rust
// contracts/skill-registry/src/lib.rs
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{serde_json, AccountId, PanicOnDefault, Promise};

#[near_sdk::near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct SkillRegistry {
    // Skill manifests (name -> metadata)
    skills: LookupMap<String, SkillMetadata>,

    // Agent skills (agent -> list of skills)
    agent_skills: LookupMap<AccountId, Vec<String>>,

    // Skill reputation (skill -> ratings)
    skill_ratings: LookupMap<String, Vec<Rating>>,

    // Usage tracking
    usage_stats: LookupMap<String, UsageStats>,
}

#[derive(BorshDeserialize, BorshSerialize, serde::Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SkillMetadata {
    pub name: String,
    pub version: String,
    pub author: AccountId,
    pub ipfs_hash: String,  // Full skill package
    pub checksum: String,   // Verify integrity
    pub price: Option<u128>, // NEAR yoctoNEAR
    pub tags: Vec<String>,
}

#[near_sdk::near_bindgen]
impl SkillRegistry {
    #[payable(deposit)]
    pub fn register_skill(&mut self, metadata: SkillMetadata) {
        // Verify skill author signature
        // Store skill metadata
        // Return skill ID
    }

    pub fn discover_skills(&self, tags: Vec<String>) -> Vec<SkillMetadata> {
        // Search by tags
    }

    pub fn find_agents_with_skill(&self, skill: String) -> Vec<AccountId> {
        // Find agents offering this skill
    }

    pub fn rate_skill(&mut self, skill: String, rating: u8, comment: String) {
        // Add rating
    }
}
```

### 2. Skill Discovery Protocol (P2P)

```rust
// src/skills/protocol.rs
use libp2p::{gossipsub, kad};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct SkillAdvertisement {
    pub agent_id: AccountId,
    pub skill_name: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub pricing: Option<SkillPricing>,
    pub peer_id: PeerId,
}

pub struct SkillDiscovery {
    gossipsub: gossipsub::Behaviour,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

impl SkillDiscovery {
    pub async fn advertise_skill(&self, skill: SkillAdvertisement) {
        // Publish to gossipsub topic: "gork-skills"
        // Store in DHT for discovery
    }

    pub async fn find_skill(&self, query: SkillQuery) -> Vec<SkillAdvertisement> {
        // Query DHT
        // Ask gossipsub peers
    }
}
```

### 3. Skill Execution Engine

```rust
// src/skills/executor.rs
pub struct SkillExecutor {
    workspace: PathBuf,
    sandbox: Sandbox,
}

impl SkillExecutor {
    pub async fn execute_skill(
        &self,
        skill: &SkillPackage,
        capability: &str,
        input: Value,
    ) -> Result<SkillOutput> {
        // 1. Verify skill integrity
        skill.verify_checksum()?;

        // 2. Load skill into sandbox
        self.sandbox.load(skill)?;

        // 3. Execute capability
        let output = self.sandbox.execute(capability, input).await?;

        // 4. Track usage
        self.track_usage(skill, capability).await?;

        Ok(output)
    }
}

// Sandbox types
pub enum Sandbox {
    Docker(DockerSandbox),
    WASM(WASMSandbox),
    Process(ProcessSandbox),
}
```

## New CLI Commands

```bash
# Skill management
gork-agent skills list
gork-agent skills search <query>
gork-agent skills inspect <skill>
gork-agent skills publish <path>
gork-agent skills test <path>

# Agent discovery
gork-agent discover find-agents --skill <skill>
gork-agent discover query-agents --capability <cap>

# Skill execution
gork-agent execute \
  --agent <peer-id> \
  --skill <skill> \
  --capability <cap> \
  --input <json>

# Marketplace
gork-agent marketplace browse
gork-agent marketplace rate
gork-agent marketplace stats
```

## Directory Structure

```
~/.gork-agent/
├── skills/              # Local skill packages
│   ├── my-analysis/
│   │   ├── skill.yaml
│   │   ├── instructions.md
│   │   └── resources/
├── skill-cache/         # Downloaded skills
├── executions/          # Execution history
└── marketplace/         # Marketplace data
```

## Examples

### Publish a Skill

```bash
# Create skill
mkdir -p ~/my-skill
cd ~/my-skill

# skill.yaml
cat > skill.yaml <<EOF
name: web-scraper
version: 1.0.0
description: Scrape websites and extract data
author: myaccount.near
tags: [scraping, web, data-extraction]
capabilities:
  - name: scrape-url
    description: Extract data from a URL
    input:
      type: object
      properties:
        url:
          type: string
        selectors:
          type: object
EOF

# Instructions
cat > instructions.md <<EOF
# Web Scraper Skill

## Usage

Scrape a URL:
```yaml
capability: scrape-url
input:
  url: https://example.com
  selectors:
    title: "h1"
    links: "a[href]"
```
EOF

# Publish
gork-agent skills publish ./
```

### Use a Skill from Another Agent

```bash
# Find agents with web scraping skill
gork-agent discover find-agents --skill web-scraper

# Use the skill
gork-agent execute \
  --agent bob.near \
  --skill web-scraper \
  --capability scrape-url \
  --input '{"url": "https://example.com", "selectors": {"title": "h1"}}'
```

## Benefits

1. **Decentralized Marketplace** - No central authority
2. **Verified Identity** - NEAR account verification
3. **Reputation System** - Rate and review skills
4. **Monetization** - Charge for skill usage
5. **Interoperability** - Compatible with Agent Skills standard
6. **P2P Discovery** - Find skills without central servers
7. **Privacy** - Direct agent-to-agent communication
8. **Composability** - Chain multiple skills together

## Next Steps

1. Implement NEAR skill registry contract
2. Add skill discovery protocol to P2P layer
3. Build skill execution sandbox
4. Create CLI commands for skill management
5. Develop marketplace interface
6. Add reputation and rating system
