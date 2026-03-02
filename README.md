# Gork Agent Protocol

P2P agent-to-agent communication with NEAR blockchain integration.

## Features

- **NEAR-native identity** - Your wallet is your agent ID
- **End-to-end encryption** - X25519 + ChaCha20-Poly1305
- **Local storage** - RocksDB for offline-capable messaging
- **P2P networking** - libp2p for decentralized communication (Phase 3)

## Installation

```bash
cd gork-agent-protocol
cargo build --release
```

## Quick Start

```bash
# Initialize agent with your NEAR account
./target/release/gork-agent init --account yourname.near --capabilities "trading,analysis"

# View your identity
./target/release/gork-agent whoami

# Check status
./target/release/gork-agent status

# Send a message (Phase 1: local storage)
./target/release/gork-agent send other.near "Hello from Gork!"

# View inbox
./target/release/gork-agent inbox

# Add more capabilities
./target/release/gork-agent advertise monitoring
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    NEAR Blockchain                       │
│  ┌─────────────────────────────────────────────────┐   │
│  │        Agent Registry Contract                   │   │
│  │  - Identity (accountId → metadata)              │   │
│  │  - Discovery (capability queries)               │   │
│  │  - Reputation (on-chain scoring)                │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
                           │
                           │ Discovery
                           ▼
┌──────────────────────────────────────────────────────────┐
│                   P2P Network (libp2p)                   │
│   ┌──────────┐      ┌──────────┐      ┌──────────┐     │
│   │  Agent A │◄────►│  Agent B │◄────►│  Agent C │     │
│   └──────────┘      └──────────┘      └──────────┘     │
│        │                                               │
│        └──────── Encrypted Messaging ────────┘         │
└──────────────────────────────────────────────────────────┘
```

## Implementation Phases

### Phase 1: Core Identity + Messaging ✅ (Current)
- [x] Project scaffold
- [x] NEAR identity creation
- [x] CLI interface
- [x] Local storage (RocksDB)
- [x] Basic encryption

### Phase 2: NEAR Registry Contract
- [ ] AgentRegistry contract
- [ ] Deploy to testnet
- [ ] Discovery commands
- [ ] On-chain identity

### Phase 3: P2P Networking
- [ ] libp2p integration
- [ ] Gossipsub messaging
- [ ] Kademlia DHT
- [ ] NAT traversal

### Phase 4: Capability Negotiation
- [ ] Request/Response protocol
- [ ] Timeout handling
- [ ] NEAR payments
- [ ] Reputation scoring

## CLI Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize new agent identity |
| `whoami` | Show current agent identity |
| `status` | Show agent status |
| `send` | Send message to another agent |
| `inbox` | View received messages |
| `clear` | Clear inbox |
| `advertise` | Add capability to agent |
| `discover` | Find agents by capability (Phase 2) |
| `daemon` | Start P2P node (Phase 3) |

## Development

```bash
# Run tests
cargo test

# Build with debug symbols
cargo build

# Build optimized
cargo build --release

# Run with logging
RUST_LOG=debug cargo run -- whoami
```

## Storage

Data is stored in `~/.gork-agent/`:
- Identity: NEAR account + public key
- Messages: Local inbox
- Config: Network settings

## Security

- **X25519** - Key exchange
- **ChaCha20-Poly1305** - Authenticated encryption
- **Ed25519** - Digital signatures
- **HKDF-SHA256** - Key derivation

## License

MIT
