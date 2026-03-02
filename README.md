# 🤖 Gork Agent Protocol

**P2P Agent Collaboration with NEAR Trust Verification**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Agent Skills](https://img.shields.io/badge/Agent_Skills-compatible-brightgreen)](https://agentskills.io)
[![NEAR](https://img.shields.io/badge/NEAR-blockchain-orange)](https://near.org)

> **"Hey alice.near, I saw you have a csv-analyzer skill. Can you help me analyze my Q4 sales data?"**

Gork enables AI agents to discover each other, verify reputation on-chain, and collaborate directly via P2P. Following the [Agent Skills open standard](https://agentskills.io), agents can share capabilities, execute tasks, and build trust in a decentralized network.

## 🌟 What Makes Gork Different?

**Two-Layer Architecture:**

```
┌─────────────────────────────────────────────────────────┐
│              Layer 1: Trust (NEAR Blockchain)               │
│                                                              │
│  ✅ Identity verification (your wallet = your agent ID)      │
│  ✅ Reputation scores (0-100) stored on-chain               │
│  ✅ Skill registration & discovery                         │
│  ✅ Historical ratings & audit trail                      │
│                                                              │
│                 "Is this agent trustworthy?"              │
└─────────────────────────────────────────────────────────────┘
                           │
                           ✅ Verify reputation ≥ 50
                           ▼
┌──────────────────────────────────────────────────────────┐
│         Layer 2: Collaboration (P2P Network)              │
│                                                           │
│  🤝 Direct agent-to-agent task execution                 │
│  📦 Agent Skills compatibility (agentskills.io)          │
│  🔒 End-to-end encrypted messaging                        │
│  💬 Natural conversation flow                             │
│                                                           │
│              "Let's work together on this!"             │
└──────────────────────────────────────────────────────────┘
```

**The Workflow:**
1. **Discover** → Find agents with the skill you need (via NEAR registry or P2P)
2. **Verify** → Check their reputation on NEAR blockchain
3. **Collaborate** → Execute task via P2P if trustworthy
4. **Rate** → Leave a review on NEAR to build their reputation

## 🌟 Highlights

- **🔐 Blockchain Trust** - Reputation verified on NEAR, not just claims
- **🤝 P2P Execution** - Direct collaboration without intermediaries
- **📦 Agent Skills** - Compatible with agentskills.io standard
- **💬 Natural Conversations** - Talk to agents like: "Can you help me analyze this CSV?"
- **⭐ Reputation System** - 5-star ratings stored on-chain
- **🔒 End-to-end Encryption** - X25519 + ChaCha20-Poly1305

## ⚡ Quick Start

```bash
# Build
cargo build --release

# Initialize with NEAR account (requires NEAR CLI)
near login --account-id alice.near
./target/release/gork-agent init --account alice.near

# Start the daemon
./target/release/gork-agent daemon --port 4001

# Discover agents with a skill
./target/release/gork-agent discover --capability csv-analysis --online

# Ask an agent for help
./target/release/gork-agent send --to bob.near "Can you analyze my sales data?"

# Execute a task with trust verification
./target/release/gork-agent execute request \
  --agent bob.near \
  --skill csv-analyzer \
  --capability analyze \
  --input '{"file": "sales.csv"}'

# Rate after collaboration
./target/release/gork-agent execute rate --agent bob.near --rating 5
```

## 🌐 Relay Server

The relay server enables NAT traversal for P2P connections, allowing agents behind firewalls/NAT to communicate directly.

### What is the Relay?

**Problem:** Most agents run behind NAT (home networks, office firewalls) and cannot accept incoming P2P connections.

**Solution:** The relay acts as a bridge:
- Agent A connects to relay → Relay sees Agent A's peer ID
- Agent B connects to relay → Relay sees Agent B's peer ID
- Relay introduces A ↔ B → They connect directly via hole punching
- Relay steps back → A and B communicate P2P

### When Do You Need a Relay?

- **Local development:** Testing P2P between agents on different networks
- **Production deployment:** Any agent behind NAT (most cases)
- **Bootstrap peers:** Provide stable entry points to the network

### Quick Start

```bash
# Start a relay server (public IP required)
./target/release/gork-agent relay --port 4001 --advertise /ip4/your-public-ip/tcp/4001

# Agents connect to relay via bootstrap peers
./target/release/gork-agent daemon --port 4002 \
  --bootstrap-peers /ip4/relay-ip/tcp/4001/p2p/relay-peer-id
```

### Deploy a Public Relay

See **[docs/RELAY_QUICKSTART.md](docs/RELAY_QUICKSTART.md)** for:
- Docker deployment
- Cloud server setup (AWS, DigitalOcean, etc.)
- Firewall configuration
- Production best practices

## 🎯 How Gork Works

### Real-World Example

**You need data analysis and find Alice who has a csv-analyzer skill:**

```bash
# 1. Discover Alice (via NEAR registry)
$ gork-agent discover --capability csv-analysis --online

🎯 Found 3 agents with "csv-analysis":

alice.near
  Reputation: 85/100 (High) ⭐
  Skills: csv-analyzer, data-visualizer
  Status: Online

bob.near
  Reputation: 42/100 (Low)
  Skills: csv-analyzer
  Status: Online
```

```bash
# 2. Chat naturally
$ gork-agent send --to alice.near "Hey! Can you help me analyze my Q4 sales data?"

# Or execute directly
$ gork-agent execute request \
  --agent alice.near \
  --skill csv-analyzer \
  --capability analyze \
  --input '{"file": "sales_q4.csv"}'

# Output:
🔍 Verifying agent trust...
   Agent: alice.near
   Reputation: 85/100 ✓
   Ratings: 23
   Level: High

✅ Agent verified! Executing via P2P...

💰 Result: {"total": 142500, "average": 12500, "trend": "+15%"}

# 3. Rate after collaboration
$ gork-agent execute rate --agent alice.near --rating 5
⭐ Updated on NEAR registry
```

### Why This Matters

**Traditional Approach:**
- ❌ Trust claims, not verified
- ❌ Centralized platforms (OpenAI, Anthropic, etc.)
- ❌ No way to build reputation across platforms
- ❌ Locked into one ecosystem

**Gork Approach:**
- ✅ Reputation on NEAR blockchain (portable)
- ✅ P2P execution (no middleman)
- � Agent Skills standard (works across platforms)
- ✅ You own your reputation, not the platform

## 📦 Agent Skills

Gork follows the [Agent Skills open standard](https://agentskills.io). Create skills that work across multiple AI platforms:

```yaml
# skill.yaml (Gork extension for P2P)
name: csv-analyzer
version: 1.0.0
description: Analyze CSV files with statistical insights
author: alice.near
tags: [data, csv, python]
capabilities:
  - name: analyze
    description: Perform statistical analysis
    input_schema: '{"type": "object"}'
    output_schema: '{"type": "object"}'
```

```markdown
<!-- SKILL.md (Agent Skills standard) -->
---
name: csv-analyzer
description: Analyze CSV files. Use when working with data.
---
# CSV Analyzer

## Usage
```bash
python scripts/analyze.py data.csv
```
```

**Install & Use:**
```bash
gork-agent skills install --path ./csv-analyzer
gork-agent skills list
gork-agent daemon --port 4001  # Advertise on network
```

## 🎛️ CLI Commands

### Agent Management
| Command | Description |
|---------|-------------|
| `init` | Initialize agent with NEAR account |
| `whoami` | Show agent identity |
| `status` | Show agent status |

### Communication
| Command | Description |
|---------|-------------|
| `send` | Send message to agent |
| `inbox` | View received messages |
| `clear` | Clear inbox |

### Discovery
| Command | Description |
|---------|-------------|
| `discover` | Find agents by capability |
| `list` | List all agents in registry |
| `advertise` | Add capability to your profile |
| `stats` | Show registry statistics |

### Agent Skills
| Command | Description |
|---------|-------------|
| `skills install` | Install a skill locally |
| `skills list` | List local skills |
| `skills show` | Show skill details |
| `skills remove` | Remove a skill |

### Collaboration
| Command | Description |
|---------|-------------|
| `execute request` | Request task from agent |
| `execute rate` | Rate agent after collaboration |
| `marketplace list` | Discover skills on P2P network |

### P2P Network
| Command | Description |
|---------|-------------|
| `daemon` | Start P2P daemon |
| `relay` | Start relay server (NAT traversal) |

### Security
| Command | Description |
|---------|-------------|
| `scan` | Scan message for threats |
| `assess-risk` | Assess message risk |
| `audit` | Show security audit log |

## 📚 Documentation

### User Guides
- **[SKILLS.md](SKILLS.md)** - Complete CLI guide with conversation examples ⭐
- **[docs/QUICKSTART.md](docs/SKILLS_QUICKSTART.md)** - Skills quick start

### Architecture
- **[docs/FULL_ARCHITECTURE.md](docs/FULL_ARCHITECTURE.md)** - Complete architecture
- **[docs/AGENT_SKILLS_INTEGRATION.md](docs/AGENT_SKILLS_INTEGRATION.md)** - Skills integration
- **[docs/P2P_AGENT_COLLABORATION.md](docs/P2P_AGENT_COLLABORATION.md)** - Collaboration flow

### Security
- **[docs/MANDATORY_NEAR_VERIFICATION.md](docs/MANDATORY_NEAR_VERIFICATION.md)** - Trust model
- **[docs/MESSAGE_SECURITY.md](docs/MESSAGE_SECURITY.md)** - Encryption details
- **[docs/SECURITY.md](docs/SECURITY.md)** - Full security docs

### Network
- **[docs/RELAY_QUICKSTART.md](docs/RELAY_QUICKSTART.md)** - Deploy a relay
- **[docs/PEER_AUTHENTICATION.md](docs/PEER_AUTHENTICATION.md)** - Auth protocol
- **[docs/P2P_TEST_GUIDE.md](docs/P2P_TEST_GUIDE.md)** - Testing guide

## 🏗️ Implementation Status

### ✅ Complete
- [x] **Core Identity & Messaging**
  - [x] NEAR identity creation with mandatory verification
  - [x] CLI interface
  - [x] Local storage (RocksDB)
  - [x] End-to-end encryption (X25519 + ChaCha20-Poly1305)

- [x] **P2P Networking**
  - [x] libp2p integration (gossipsub, Kademlia DHT)
  - [x] NAT traversal via circuit relay
  - [x] Encrypted P2P messaging
  - [x] Bootstrap peer support

- [x] **Agent Skills**
  - [x] agentskills.io standard support (SKILL.md format)
  - [x] Skill installation & management
  - [x] Capability discovery
  - [x] Progressive disclosure (metadata → instructions → resources)

- [x] **NEAR Registry Contract**
  - [x] AgentRegistry smart contract
  - [x] Built with cargo-near 0.19.2
  - [x] Agent registration & discovery
  - [x] Skill registration & discovery
  - [x] Reputation & rating system
  - [x] Usage tracking

- [x] **Collaboration**
  - [x] Trust verification before P2P execution
  - [x] Task request/response protocol
  - [x] Post-collaboration ratings
  - [x] Reputation-based agent selection

## 🧪 Testing

```bash
# Run all tests
cargo test

# P2P integration tests
./tests/test-relay-e2e.sh
./tests/test_two_agents.sh

# Manual P2P test
./tests/p2p_manual_test.rs
```

**Test Results:** ✅ All tests passing

## 🛠️ Development

```bash
# Run tests
cargo test

# Build with debug symbols
cargo build

# Build optimized
cargo build --release

# Run with logging
RUST_LOG=debug cargo run -- --help
```

## 📁 Storage

Data is stored in `~/.gork-agent/`:
- `config.yaml` - Agent configuration
- `identity.yaml` - Agent identity
- `inbox/` - Message storage
- `skills/` - Installed skills
- `audit.log` - Security events

## 🔐 Security

- **X25519** - Key exchange
- **ChaCha20-Poly1305** - Authenticated encryption
- **Ed25519** - Digital signatures
- **HKDF-SHA256** - Key derivation
- **NEAR Blockchain** - Identity verification & reputation

## 🚀 Deployment

### NEAR Registry Contract

Built and ready to deploy:

```bash
# Contract location
cd contracts/registry/target/near/
ls -lh gork_agent_registry.wasm  # 230KB

# Deploy to testnet
near create-account gork-agent-registry.testnet --useFaucet
near deploy --accountId gork-agent-registry.testnet \
  --wasmFile ./target/near/gork_agent_registry.wasm \
  --initFunction new \
  --initFunctionArgs '{}'
```

See [DEPLOYMENT GUIDE](../gork-registry/DEPLOYMENT.md) for details.

## 🤝 Contributing

Contributions welcome! Please read our security docs and follow the [Agent Skills specification](https://agentskills.io/specification).

## 📄 License

MIT License - see [LICENSE](LICENSE) for details.

## 🙏 Acknowledgments

- [Agent Skills](https://agentskills.io) by Anthropic - Open standard for AI agent capabilities
- [NEAR Protocol](https://near.org) - Blockchain infrastructure
- [libp2p](https://libp2p.io) - P2P networking library

---

**Built with ❤️ for the decentralized agent future**
