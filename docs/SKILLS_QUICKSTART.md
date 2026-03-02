# Agent Skills CLI Quick Reference

## Command Structure

```bash
gork-agent [global-options] <command> [subcommand] [options]
```

## Global Options

- `-n, --network <NETWORK>` - Network (testnet/mainnet) [default: testnet]
- `-r, --registry <REGISTRY>` - Registry contract ID

---

## Skills Commands

### Publish a Skill

```bash
gork-agent skills publish --path <path> [--skip-ipfs]
```

**Example:**
```bash
gork-agent skills publish --path ./csv-analyzer/
```

### Search Skills

```bash
gork-agent skills search [--tag <tag>] [--query <query>] [--min-rating <rating>] [--limit <n>]
```

**Examples:**
```bash
# Search by tag
gork-agent skills search --tag data-analysis

# Search by query
gork-agent skills search --query csv

# With minimum rating
gork-agent skills search --tag python --min-rating 4.0
```

### Inspect a Skill

```bash
gork-agent skills inspect --skill <skill-id>
```

**Example:**
```bash
gork-agent skills inspect --skill csv-analyzer@1.0.0
```

### List Agent's Skills

```bash
gork-agent skills list-agent --agent <account-id>
```

**Example:**
```bash
gork-agent skills list-agent --agent alice.near
```

### Find Agents with Skill

```bash
gork-agent skills find-agents --skill <skill-name>
```

**Example:**
```bash
gork-agent skills find-agents --skill csv-analyzer
```

### Show Top Skills

```bash
gork-agent skills top [--limit <n>]
```

**Example:**
```bash
gork-agent skills top --limit 20
```

---

## Execute Commands

### Execute a Skill

```bash
gork-agent execute --agent <account> --skill <skill> --capability <cap> [--input <json>]
```

**Example:**
```bash
gork-agent execute \
  --agent alice.near \
  --skill csv-analyzer \
  --capability analyze \
  --input '{"file_path": "data.csv", "columns": ["age", "salary"]}'
```

---

## Marketplace Commands

### Rate a Skill

```bash
gork-agent marketplace rate --skill <skill-id> <rating>
```

**Example:**
```bash
gork-agent marketplace rate --skill csv-analyzer@1.0.0 5
```

### View Skill Statistics

```bash
gork-agent marketplace stats --skill <skill-id>
```

**Example:**
```bash
gork-agent marketplace stats --skill csv-analyzer@1.0.0
```

### Show Trending Skills

```bash
gork-agent marketplace trending [--limit <n>]
```

**Example:**
```bash
gork-agent marketplace trending --limit 10
```

---

## Skill Package Structure

```
my-skill/
├── skill.yaml          # Required: Skill manifest
└── code/               # Optional: Implementation code
    ├── main.py
    └── requirements.txt
```

## skill.yaml Format

```yaml
name: my-skill
version: 1.0.0
description: Skill description
author: account.near

tags:
  - category1
  - category2

capabilities:
  - name: capability-name
    description: What it does
    input_schema: |
      {"type": "object", "properties": {...}}
    output_schema: |
      {"type": "object", "properties": {...}}
    examples:
      - '{"example": "input"}'

requirements:
  timeout_secs: 30
  memory_mb: 512
  dependencies:
    - python>=3.9

pricing:
  free_tier_calls_per_day: 100
  cost_per_call_yocto: "1000000000000"
```

---

## Common Workflows

### Publishing a New Skill

```bash
# 1. Create skill package
mkdir my-skill
cd my-skill

# 2. Create skill.yaml
cat > skill.yaml <<EOF
name: my-skill
version: 1.0.0
description: My awesome skill
tags: [example]
capabilities:
  - name: do-something
    description: Does something
    input_schema: '{}'
    output_schema: '{}'
    examples: []
requirements:
  timeout_secs: 30
  memory_mb: 512
  dependencies: []
EOF

# 3. Publish
gork-agent skills publish --path ./
```

### Finding and Using a Skill

```bash
# 1. Search for skills
gork-agent skills search --tag data-analysis

# 2. Inspect interesting skill
gork-agent skills inspect --skill csv-analyzer@1.0.0

# 3. Find agents with skill
gork-agent skills find-agents --skill csv-analyzer

# 4. Execute skill
gork-agent execute \
  --agent alice.near \
  --skill csv-analyzer \
  --capability analyze \
  --input '{"file_path": "data.csv"}'

# 5. Rate the skill
gork-agent marketplace rate csv-analyzer@1.0.0 5
```

---

## Tips

- Use semantic versioning (1.0.0, 1.1.0, 2.0.0)
- Be specific with tags for better discoverability
- Include examples in capabilities
- Set appropriate timeout and memory limits
- Use pricing to monetize popular skills
- Always test locally before publishing

---

## Getting Help

```bash
# General help
gork-agent --help

# Command-specific help
gork-agent skills --help
gork-agent skills publish --help
gork-agent execute --help
```
