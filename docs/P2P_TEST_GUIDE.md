# P2P Testing Guide

This guide shows how to test two agents communicating via P2P.

## Quick Test (Automated)

Run the smoke test to verify basic functionality:

```bash
cargo test --test p2p_smoke_test -- --nocapture
```

Expected output:
```
✅ Agent network created successfully!
   Peer ID: 12D3KooW...
✅ Listening on: /ip4/0.0.0.0/tcp/0
✅ Test PASSED! P2P network creation works correctly.
```

## Manual Testing (Two Agents)

### Terminal 1 - Agent 1

```bash
# Initialize agent
cargo run -- init --account alice.test --capabilities "compute,storage"

# Start daemon
cargo run -- daemon
```

Output will show:
```
🚀 Starting Gork Agent P2P Daemon
🤖 Agent: alice.test
🌐 Initializing P2P network...
📡 Listening on: /ip4/0.0.0.0/tcp/4001
   Peer ID: 12D3KooW...
✅ Daemon started successfully!
```

### Terminal 2 - Agent 2

```bash
# Initialize agent
cargo run -- init --account bob.test --capabilities "compute"

# Start daemon
cargo run -- daemon
```

### Testing Connection

1. **Get Agent 2's info:**
   ```bash
   cargo run -- whoami
   ```

2. **Agent 1 should show:**
   ```
   🟢 Peer connected: [Agent2's Peer ID]
   ```

3. **Send a message:**
   ```bash
   cargo run -- send bob.test "Hello from Alice!"
   ```

## Current Status

✅ **Build fixed** - P2P module compiles successfully with libp2p 0.55
✅ **Unit tests passing** - All network module tests pass
✅ **Smoke test passing** - Basic P2P network creation works
✅ **Daemon functional** - Can start P2P daemon and listen for connections

## Test Results Summary

| Test | Status |
|------|--------|
| `test_network_config_default` | ✅ PASS |
| `test_parse_multiaddr` | ✅ PASS |
| `test_create_p2p_message` | ✅ PASS |
| `test_network_creation` | ✅ PASS |
| `test_create_agent_network` | ✅ PASS |

## Architecture

The P2P implementation uses:
- **libp2p 0.55** - P2P networking library
- **Gossipsub** - Pub/sub messaging protocol
- **Kademlia DHT** - Distributed hash table for peer discovery
- **Identify** - Protocol for exchanging peer information
- **Ping** - Connection health monitoring
- **TCP + Noise + Yamux** - Transport stack

## Next Steps

To enable full two-agent messaging:
1. The `run()` event loop needs to be active (runs in `daemon` command)
2. Both agents need to be on the same network
3. Bootstrap peers or manual dialing required for initial connection
4. Gossipsub message propagation needs time to establish

The build fixes ensure all components compile and basic connectivity works.
