# Gork Agent Protocol

P2P agent-to-agent communication with NEAR blockchain integration.

## Features

- **NEAR-native identity** - Your wallet is your agent ID
- **Mandatory NEAR verification** - All agents must prove NEAR account ownership
- **End-to-end encryption** - X25519 + ChaCha20-Poly1305
- **Local storage** - RocksDB for offline-capable messaging
- **P2P networking** - libp2p with gossipsub, Kademlia DHT, and relay support
- **NAT traversal** - Circuit relay for peers behind firewalls

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

## Implementation Status

### ✅ Phase 1: Core Identity + Messaging (Complete)
- [x] Project scaffold
- [x] NEAR identity creation
- [x] CLI interface
- [x] Local storage (RocksDB)
- [x] Basic encryption

### ✅ Phase 3: P2P Networking (Complete)
- [x] libp2p integration
- [x] Gossipsub pub/sub messaging
- [x] Kademlia DHT for peer discovery
- [x] NAT traversal via relay
- [x] Bootstrap peer support

### 🚧 Phase 2: NEAR Registry Contract (Pending)
- [ ] AgentRegistry contract
- [ ] Deploy to testnet
- [ ] Discovery commands
- [ ] On-chain identity

### 🚧 Phase 4: Capability Negotiation (Pending)
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
| `daemon` | Start P2P node with optional bootstrap peers |
| `relay` | Start P2P relay server for NAT traversal |

## Documentation

- **[Relay Quick Start](docs/RELAY_QUICKSTART.md)** - Deploy a P2P relay
- **[Relay Design](docs/RELAY_DESIGN.md)** - Architecture and theory
- **[Relay Test Results](docs/RELAY_TEST_RESULTS.md)** - E2E test results
- **[Mandatory NEAR Verification](docs/MANDATORY_NEAR_VERIFICATION.md)** - Security model
- **[Peer Authentication](docs/PEER_AUTHENTICATION.md)** - Authentication protocol
- **[Message Security](docs/MESSAGE_SECURITY.md)** - Encryption details
- **[Security Overview](docs/SECURITY.md)** - Full security documentation

## Tests

See the `tests/` directory for automated test scripts:

```bash
# End-to-end relay test (tests peer connection via relay)
./tests/test-relay-e2e.sh

# Two-agent local communication test
./tests/test_two_agents.sh
```

**Test Results:** ✅ All tests passing - relay successfully facilitates peer discovery and NAT traversal

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
