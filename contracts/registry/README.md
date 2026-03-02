# Gork Agent Registry Contract

NEAR smart contract for agent discovery and reputation.

## Contract Methods

### Registration

```bash
# Register agent
near call registry.testnet register '{
  "name": "Gork",
  "capabilities": ["trading", "monitoring", "analysis"],
  "endpoint": "/ip4/1.2.3.4/tcp/4001",
  "public_key": "base58encodedkey",
  "description": "Autonomous trading agent"
}' --accountId your.near --networkId testnet
```

### Discovery

```bash
# Find trading agents
near view registry.testnet discover '{
  "capability": "trading",
  "online_only": true,
  "limit": 10
}' --networkId testnet

# Get specific agent
near view registry.testnet get_agent '{"account_id": "agent.near"}' --networkId testnet

# Get all agents
near view registry.testnet get_all_agents '{"from_index": 0, "limit": 50}' --networkId testnet
```

### Reputation

```bash
# Rate an agent
near call registry.testnet rate_agent '{
  "agent_id": "other.near",
  "score": 85
}' --accountId your.near --networkId testnet
```

### Management

```bash
# Heartbeat (keep online status)
near call registry.testnet heartbeat --accountId your.near --networkId testnet

# Update capabilities
near call registry.testnet update_capabilities '{
  "capabilities": ["trading", "analysis", "forecasting"]
}' --accountId your.near --networkId testnet

# Set offline
near call registry.testnet set_offline --accountId your.near --networkId testnet

# Unregister
near call registry.testnet unregister --accountId your.near --networkId testnet
```

## Build

```bash
cd contracts/registry
rustup override set 1.86.0
cargo near build non-reproducible-wasm
```

## Deploy

```bash
# Create sub-account
near create-account registry.yourname.testnet \
  --masterAccount yourname.testnet \
  --initialBalance 10 \
  --networkId testnet

# Deploy
near deploy registry.yourname.testnet \
  target/near/gork_agent_registry.wasm \
  --networkId testnet
```

## Contract Schema

### AgentMetadata

```json
{
  "account_id": "agent.near",
  "name": "Gork",
  "capabilities": ["trading", "monitoring"],
  "endpoint": "/ip4/1.2.3.4/tcp/4001",
  "public_key": "base58...",
  "reputation": 85,
  "rating_count": 10,
  "last_seen": 1709123456789000000,
  "description": "Autonomous trading agent",
  "online": true
}
```

## Gas Costs (Estimated)

| Method | Gas |
|--------|-----|
| register | ~5 TGas |
| discover | ~2 TGas |
| get_agent | ~1 TGas |
| rate_agent | ~3 TGas |
| heartbeat | ~1 TGas |
