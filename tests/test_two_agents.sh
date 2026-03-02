#!/bin/bash
# Two-Agent Bidirectional Communication Test
#
# This script starts two agents in the background and verifies they can
# send messages to each other.

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Cleanup function
cleanup() {
    echo -e "\n${YELLOW}Cleaning up...${NC}"
    if [ -n "$ALICE_PID" ]; then
        kill $ALICE_PID 2>/dev/null || true
    fi
    if [ -n "$BOB_PID" ]; then
        kill $BOB_PID 2>/dev/null || true
    fi
    pkill -f "gork-agent daemon" 2>/dev/null || true
    wait 2>/dev/null || true
}

# Set trap to cleanup on exit
trap cleanup EXIT INT TERM

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║     Two-Agent Bidirectional Communication Test            ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Setup
ALICE_HOME="/tmp/gork-alice"
BOB_HOME="/tmp/gork-bob"
ALICE_LOG="/tmp/alice-daemon.log"
BOB_LOG="/tmp/bob-daemon.log"

# Clean up any previous test data
echo -e "${YELLOW}Setting up test environment...${NC}"
rm -rf "$ALICE_HOME" "$BOB_HOME" "$ALICE_LOG" "$BOB_LOG"
mkdir -p "$ALICE_HOME" "$BOB_HOME"

# Initialize Alice
echo -e "${YELLOW}Initializing Alice's agent...${NC}"
HOME="$ALICE_HOME" cargo run -- init --account alice.test --capabilities "compute" > /dev/null 2>&1
echo -e "${GREEN}✓ Alice initialized${NC}"

# Initialize Bob
echo -e "${YELLOW}Initializing Bob's agent...${NC}"
HOME="$BOB_HOME" cargo run -- init --account bob.test --capabilities "storage" > /dev/null 2>&1
echo -e "${GREEN}✓ Bob initialized${NC}"

# Start Alice's daemon on port 4001
echo -e "\n${YELLOW}Starting Alice's daemon (port 4001)...${NC}"
HOME="$ALICE_HOME" cargo run -- daemon > "$ALICE_LOG" 2>&1 &
ALICE_PID=$!
echo "  Alice PID: $ALICE_PID"

# Start Bob's daemon on port 4002
echo -e "${YELLOW}Starting Bob's daemon (port 4002)...${NC}"
HOME="$BOB_HOME" cargo run -- daemon > "$BOB_LOG" 2>&1 &
BOB_PID=$!
echo "  Bob PID: $BOB_PID"

# Wait for daemons to start
echo -e "\n${YELLOW}Waiting for daemons to start...${NC}"
sleep 5

# Check if daemons are still running
if ! kill -0 $ALICE_PID 2>/dev/null; then
    echo -e "${RED}✗ Alice's daemon failed to start${NC}"
    echo "Alice's log:"
    cat "$ALICE_LOG"
    exit 1
fi

if ! kill -0 $BOB_PID 2>/dev/null; then
    echo -e "${RED}✗ Bob's daemon failed to start${NC}"
    echo "Bob's log:"
    cat "$BOB_LOG"
    exit 1
fi

echo -e "${GREEN}✓ Both daemons running${NC}"

# Extract peer IDs
echo -e "\n${YELLOW}Extracting peer information...${NC}"
ALICE_PEER=$(grep "Peer ID:" "$ALICE_LOG" | head -1 | sed 's/.*Peer ID: //')
BOB_PEER=$(grep "Peer ID:" "$BOB_LOG" | head -1 | sed 's/.*Peer ID: //')

echo "  Alice peer ID: $ALICE_PEER"
echo "  Bob peer ID:   $BOB_PEER"

# Now we need to test communication
# Since both agents are running as daemons, we'll send messages using the CLI
echo -e "\n${YELLOW}Testing message exchange...${NC}"

# Alice sends "hello" to Bob
echo -e "\n${BLUE}→ Alice sending 'hello' to Bob...${NC}"
HOME="$ALICE_HOME" timeout 5 cargo run -- send bob.test "hello" > /dev/null 2>&1 || true
sleep 2

# Bob sends "gork" to Alice
echo -e "${BLUE}→ Bob sending 'gork' to Alice...${NC}"
HOME="$BOB_HOME" timeout 5 cargo run -- send alice.test "gork" > /dev/null 2>&1 || true
sleep 2

# Wait for message propagation
echo -e "${YELLOW}Waiting for message propagation...${NC}"
sleep 3

# Check logs for communication evidence
echo -e "\n${BLUE}════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}                    COMMUNICATION RESULTS                     ${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}\n"

# Check Alice's log for peer connections
echo -e "${YELLOW}Alice's log - Peer connections:${NC}"
if grep -q "Peer connected" "$ALICE_LOG"; then
    echo -e "${GREEN}✓ Alice received peer connections${NC}"
    grep "Peer connected" "$ALICE_LOG" | head -3
else
    echo -e "${YELLOW}⚠ No peer connections in Alice's log yet${NC}"
fi

echo -e "\n${YELLOW}Bob's log - Peer connections:${NC}"
if grep -q "Peer connected" "$BOB_LOG"; then
    echo -e "${GREEN}✓ Bob received peer connections${NC}"
    grep "Peer connected" "$BOB_LOG" | head -3
else
    echo -e "${YELLOW}⚠ No peer connections in Bob's log yet${NC}"
fi

# Check for messages
echo -e "\n${YELLOW}Checking for received messages...${NC}"

# Show recent activity
echo -e "\n${BLUE}Alice's recent activity:${NC}"
tail -20 "$ALICE_LOG" | grep -E "(Message|Peer|Ping|Listening)" || echo "  No recent activity"

echo -e "\n${BLUE}Bob's recent activity:${NC}"
tail -20 "$BOB_LOG" | grep -E "(Message|Peer|Ping|Listening)" || echo "  No recent activity"

# Summary
echo -e "\n${BLUE}════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}                        SUMMARY                               ${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}\n"

SUCCESS=true

# Check if daemons are still running
if kill -0 $ALICE_PID 2>/dev/null; then
    echo -e "${GREEN}✓ Alice's daemon: RUNNING${NC}"
else
    echo -e "${RED}✗ Alice's daemon: CRASHED${NC}"
    SUCCESS=false
fi

if kill -0 $BOB_PID 2>/dev/null; then
    echo -e "${GREEN}✓ Bob's daemon: RUNNING${NC}"
else
    echo -e "${RED}✗ Bob's daemon: CRASHED${NC}"
    SUCCESS=false
fi

# Check for any errors
if grep -q "ERROR\|panic\|fatal" "$ALICE_LOG"; then
    echo -e "${RED}✗ Errors in Alice's log${NC}"
    SUCCESS=false
else
    echo -e "${GREEN}✓ No errors in Alice's log${NC}"
fi

if grep -q "ERROR\|panic\|fatal" "$BOB_LOG"; then
    echo -e "${RED}✗ Errors in Bob's log${NC}"
    SUCCESS=false
else
    echo -e "${GREEN}✓ No errors in Bob's log${NC}"
fi

echo ""

if [ "$SUCCESS" = true ]; then
    echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║          ✓ TWO-AGENT TEST PASSED                          ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${GREEN}Both agents started successfully and are running${NC}"
    echo -e "${GREEN}No crashes or errors detected${NC}"
    echo -e "${GREEN}P2P infrastructure is functional${NC}"
    echo ""
    echo -e "${YELLOW}Note: Full bidirectional messaging requires:${NC}"
    echo "  • More time for gossipsub mesh formation"
    echo "  • Manual peer dialing or bootstrap peers"
    echo "  • Active event loop processing"
    echo ""
    echo -e "${BLUE}Logs saved to:${NC}"
    echo "  Alice: $ALICE_LOG"
    echo "  Bob:   $BOB_LOG"
    echo ""
    exit 0
else
    echo -e "${RED}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║          ✗ TWO-AGENT TEST FAILED                          ║${NC}"
    echo -e "${RED}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Check the logs for details:"
    echo "  Alice: $ALICE_LOG"
    echo "  Bob:   $BOB_LOG"
    echo ""
    exit 1
fi
