#!/bin/bash
set -e

echo "🚀 Deploying Gork Agent Registry with Web of Trust to Testnet"
echo "=============================================================="

# Configuration
CONTRACT_NAME="registry-wot.testnet"
WASM_FILE="/Users/asil/.openclaw/workspace/gork-protocol/contracts/registry/target/wasm32-unknown-unknown/release/gork_agent_registry.wasm"

# Check if WASM exists
if [ ! -f "$WASM_FILE" ]; then
    echo "❌ WASM file not found at $WASM_FILE"
    echo "Building contract..."
    cd /Users/asil/.openclaw/workspace/gork-protocol/contracts/registry
    cargo build --release --target wasm32-unknown-unknown
fi

echo "✅ Found WASM: $WASM_FILE"
echo ""

# Check account balance
echo "💰 Checking account balance..."
BALANCE=$(near view-account $CONTRACT_NAME --networkId testnet 2>&1 | grep "balance" || echo "Account not found")
echo "$BALANCE"
echo ""

# Deploy
echo "📦 Deploying contract..."
near deploy $CONTRACT_NAME $WASM_FILE --networkId testnet --force

echo ""
echo "✅ Deployment complete!"
echo ""
echo "Next steps:"
echo "  1. Initialize: near call $CONTRACT_NAME new '{}' --accountId $CONTRACT_NAME --networkId testnet"
echo "  2. Register agent: near call $CONTRACT_NAME register '{...}' --accountId YOUR_ACCOUNT --networkId testnet"
echo "  3. Endorse: near call $CONTRACT_NAME endorse_agent '{...}' --accountId YOUR_ACCOUNT --networkId testnet"
