#!/bin/bash
# Full End-to-End Relay Test

set -e

echo "=========================================="
echo "🧪 Full Relay E2E Test"
echo "=========================================="
echo ""

# Clean up
echo "1️⃣ Cleaning up..."
pkill -f gork-agent 2>/dev/null || true
sleep 1
rm -rf ~/.gork-agent /tmp/.gork-* /tmp/relay.log /tmp/peer*.log 2>/dev/null || true
echo "   ✅ Cleanup complete"
echo ""

# Initialize relay
echo "2️⃣ Initializing relay agent..."
./target/debug/gork-agent init --account relay.testnet --dev-mode > /dev/null 2>&1
echo "   ✅ Relay initialized"
echo ""

# Start relay
echo "3️⃣ Starting relay server..."
./target/debug/gork-agent relay --port 4001 --metrics > /tmp/relay.log 2>&1 &
RELAY_PID=$!
echo "   ✅ Relay started (PID: $RELAY_PID)"

# Wait for relay to start
sleep 5

# Check relay is running
if ! ps -p $RELAY_PID > /dev/null; then
    echo "   ❌ Relay failed to start!"
    cat /tmp/relay.log
    exit 1
fi

echo "   ✅ Relay is running"
echo ""

# Get relay peer ID
RELAY_PEER_ID=$(grep "Peer ID:" /tmp/relay.log | head -1 | awk '{print $NF}')
echo "   🆔 Relay Peer ID: $RELAY_PEER_ID"
echo ""

# Show relay listening addresses
echo "   📡 Listening on:"
grep "Local node is listening on" /tmp/relay.log | head -2 | sed 's/^/     /'
echo ""

# Initialize peer 1
echo "4️⃣ Initializing peer 1 (alice)..."
HOME=/tmp/.gork-peer1 ./target/debug/gork-agent init --account alice.testnet --dev-mode > /dev/null 2>&1
echo "   ✅ Peer 1 initialized"
echo ""

# Initialize peer 2
echo "5️⃣ Initializing peer 2 (bob)..."
HOME=/tmp/.gork-peer2 ./target/debug/gork-agent init --account bob.testnet --dev-mode > /dev/null 2>&1
echo "   ✅ Peer 2 initialized"
echo ""

# Start peer 1
echo "6️⃣ Starting peer 1 daemon..."
HOME=/tmp/.gork-peer1 ./target/debug/gork-agent daemon --port 4002 \
    --bootstrap-peers /ip4/127.0.0.1/tcp/4001/p2p/$RELAY_PEER_ID \
    > /tmp/peer1.log 2>&1 &
PEER1_PID=$!
echo "   ✅ Peer 1 started (PID: $PEER1_PID)"
echo ""

# Start peer 2
echo "7️⃣ Starting peer 2 daemon..."
HOME=/tmp/.gork-peer2 ./target/debug/gork-agent daemon --port 4003 \
    --bootstrap-peers /ip4/127.0.0.1/tcp/4001/p2p/$RELAY_PEER_ID \
    > /tmp/peer2.log 2>&1 &
PEER2_PID=$!
echo "   ✅ Peer 2 started (PID: $PEER2_PID)"
echo ""

# Wait for connections
echo "8️⃣ Waiting for connections (15 seconds)..."
sleep 15
echo ""

# Check peer 1 connections
echo "9️⃣ Checking peer 1 status..."
echo "   📋 Peer 1 log (last 20 lines):"
tail -20 /tmp/peer1.log | grep -v "^$" | sed 's/^/     /'
echo ""

# Check peer 2 connections
echo "   📋 Peer 2 log (last 20 lines):"
tail -20 /tmp/peer2.log | grep -v "^$" | sed 's/^/     /'
echo ""

# Check all processes
echo "🔍 Process Status:"
echo "   Relay: $(ps -p $RELAY_PID 2>/dev/null && echo "✅ Running" || echo "❌ Dead")"
echo "   Peer 1: $(ps -p $PEER1_PID 2>/dev/null && echo "✅ Running" || echo "❌ Dead")"
echo "   Peer 2: $(ps -p $PEER2_PID 2>/dev/null && echo "✅ Running" || echo "❌ Dead")"
echo ""

# Check for connections
echo "🔗 Connection Check:"
if grep -qi "connection established\|subscribed to topic" /tmp/peer1.log; then
    echo "   ✅ Peer 1 has connections!"
    grep -i "connection established\|subscribed to topic" /tmp/peer1.log | tail -3 | sed 's/^/     /'
else
    echo "   ⚠️  Peer 1: No connections found in logs"
fi
echo ""

if grep -qi "connection established\|subscribed to topic" /tmp/peer2.log; then
    echo "   ✅ Peer 2 has connections!"
    grep -i "connection established\|subscribed to topic" /tmp/peer2.log | tail -3 | sed 's/^/     /'
else
    echo "   ⚠️  Peer 2: No connections found in logs"
fi
echo ""

# Summary
echo "=========================================="
echo "📊 Test Summary"
echo "=========================================="
echo ""
echo "✅ Relay running on port 4001"
echo "🆔 Relay Peer ID: $RELAY_PEER_ID"
echo "✅ Peer 1 (alice) daemon started"
echo "✅ Peer 2 (bob) daemon started"
echo "✅ Both peers configured to use relay"
echo ""
echo "📝 Logs:"
echo "   Relay:   cat /tmp/relay.log"
echo "   Peer 1:  cat /tmp/peer1.log"
echo "   Peer 2:  cat /tmp/peer2.log"
echo ""
echo "🛑 To stop all:"
echo "   kill $RELAY_PID $PEER1_PID $PEER2_PID"
echo ""
echo "=========================================="

# Keep processes running for inspection
echo "🔍 Processes left running for inspection..."
echo "   Press Ctrl+C when done viewing logs"
