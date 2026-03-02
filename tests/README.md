# Gork Agent Protocol - Test Suite

This directory contains automated test scripts for the Gork Agent Protocol.

## Test Scripts

### Relay Tests

#### `test-relay-e2e.sh`
**Full end-to-end relay test** - Tests complete P2P communication via relay

**What it tests:**
- ✅ Relay server startup and initialization
- ✅ Peer agent initialization
- ✅ Peer connection to relay
- ✅ Peer discovery through relay
- ✅ Direct peer-to-peer connection establishment
- ✅ Gossipsub topic subscription
- ✅ Kademlia DHT peer discovery

**Usage:**
```bash
./test-relay-e2e.sh
```

**Setup:**
- Starts relay on port 4001
- Initializes 2 peer agents (alice.testnet, bob.testnet)
- Starts peer daemons on ports 4002 and 4003
- Waits 15 seconds for connections to establish
- Checks logs for successful connections

**Cleanup:**
Test processes are left running for inspection. Kill manually when done:
```bash
kill <relay-pid> <peer1-pid> <peer2-pid>
```

### Agent Communication Tests

#### `test_two_agents.sh`
Tests local communication between two agents.

#### `test_two_agents_simple.sh`
Simplified version of two-agent test.

#### `verify_two_agents.sh`
Verification script for two-agent functionality.

#### `verify_p2p.sh`
Verification script for P2P networking features.

## Test Results

All tests currently passing ✅

- Relay successfully facilitates peer discovery
- NAT traversal working via relay
- Direct peer-to-peer connections established
- Gossipsub messaging working
- Kademlia DHT operational

## Quick Test Commands

```bash
# Run full relay E2E test
./tests/test-relay-e2e.sh

# Run simple two-agent test
./tests/test_two_agents_simple.sh
```

## Test Logs

Test logs are written to `/tmp/`:
- `/tmp/relay.log` - Relay server logs
- `/tmp/peer1.log` - Peer 1 (alice) logs
- `/tmp/peer2.log` - Peer 2 (bob) logs

View logs with:
```bash
cat /tmp/relay.log
tail -f /tmp/peer1.log
```

## Development

To add new tests:

1. Create test script in `tests/` directory
2. Make it executable: `chmod +x tests/your-test.sh`
3. Document it in this README
4. Follow existing naming conventions

## Troubleshooting

### Tests failing?
- Ensure `gork-agent` is built: `cargo build`
- Check if ports 4001-4003 are available
- Verify no existing `gork-agent` processes: `pkill -f gork-agent`

### Processes left running?
Tests leave processes running for inspection. Clean up:
```bash
pkill -f gork-agent
```

### Permission denied?
Make test scripts executable:
```bash
chmod +x tests/*.sh
```
