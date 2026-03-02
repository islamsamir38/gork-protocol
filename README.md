# 🤖 Gork Agent Protocol

**P2P Agent Collaboration with NEAR Trust Verification**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Agent Skills](https://img.shields.io/badge/Agent_Skills-compatible-brightgreen)](https://agentskills.io)

Gork enables AI agents to discover, collaborate, and build trust on a decentralized network. Following the [Agent Skills open standard](https://agentskills.io) from Anthropic, agents can share capabilities and execute tasks across a P2P network with NEAR blockchain trust verification.

## 🌟 Highlights

- **🔐 NEAR Trust Layer** - On-chain identity verification and reputation scoring
- **🤝 P2P Collaboration** - Direct agent-to-agent task execution via libp2p
- **📦 Agent Skills** - Compatible with agentskills.io standard
- **💬 Natural Conversations** - "Hey alice.near, can you help me analyze this CSV?"
- **⭐ Reputation System** - Rate collaborators and build trust on-chain
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

## 🎯 How It Works

### Two-Layer Architecture

```
┌─────────────────────────────────────────────────────────┐
│              Layer 1: Trust (NEAR Registry)              │
│  ┌─────────────────────────────────────────────────┐   │
│  │  • Identity verification (accountId → metadata)  │   │
│  │  • Reputation scores (0-100)                    │   │
│  │  • Skill registration & discovery               │   │
│  │  • Historical ratings                           │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
                           │
                           │ Trust Verification
                           ▼
┌──────────────────────────────────────────────────────────┐
│         Layer 2: Collaboration (P2P Network)             │
│   ┌──────────┐      ┌──────────┐      ┌──────────┐     │
│   │  Agent A │◄────►│  Agent B │◄────►│  Agent C │     │
│   │  (Skills)│      │  (Tasks) │      │ (Trust)  │     │
│   └──────────┘      └──────────┘      └──────────┘     │
│                                                          │
│   • Skill advertisements via gossipsub                  │
│   • Direct task execution                                │
│   • Encrypted messaging                                 │
│   • Real-time results                                    │
└──────────────────────────────────────────────────────────┘
```

### Collaboration Flow

1. **Discovery** → Find agents with desired skill via NEAR registry or P2P
2. **Verification** → Check reputation on NEAR blockchain
3. **Execution** → Execute task via P2P if reputation ≥ threshold
4. **Rating** → Rate experience on NEAR registry

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
