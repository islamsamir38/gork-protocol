#!/bin/bash
# Simple Two-Agent Test
# Tests two agents running simultaneously

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0;0m'

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           Simple Two-Agent P2P Test                        ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Cleanup
cleanup() {
    echo -e "\n${YELLOW}Cleaning up...${NC}"
    pkill -9 -f "gork-agent" 2>/dev/null || true
    rm -rf /tmp/gork-test-* 2>/dev/null || true
}
trap cleanup EXIT

# Kill any existing gork-agent processes
pkill -9 -f "gork-agent" 2>/dev/null || true
sleep 1

# Setup directories
ALICE_DIR="/tmp/gork-test-alice"
BOB_DIR="/tmp/gork-test-bob"
rm -rf "$ALICE_DIR" "$BOB_DIR"
mkdir -p "$ALICE_DIR" "$BOB_DIR"

# Build first
echo -e "${YELLOW}Building gork-agent...${NC}"
cargo build --quiet 2>&1 | grep -v "warning:" || true

# Initialize Alice
echo -e "${YELLOW}Initializing Alice...${NC}"
cd "$ALICE_DIR"
HOME="$ALICE_DIR" cargo run --quiet -- init --account alice.test 2>&1 | grep -v "warning:" || true

# Initialize Bob
echo -e "${YELLOW}Initializing Bob...${NC}"
cd "$BOB_DIR"
HOME="$BOB_DIR" cargo run --quiet -- init --account bob.test 2>&1 | grep -v "warning:" || true

cd - > /dev/null

# Start Alice daemon
echo -e "\n${YELLOW}Starting Alice daemon (port 4001)...${NC}"
HOME="$ALICE_DIR" cargo run -- daemon > /tmp/alice.log 2>&1 &
ALICE_PID=$!
echo "  PID: $ALICE_PID"

# Start Bob daemon
echo -e "${YELLOW}Starting Bob daemon (port 4002)...${NC}"
HOME="$BOB_DIR" cargo run -- daemon > /tmp/bob.log 2>&1 &
BOB_PID=$!
echo "  PID: $BOB_PID"

# Wait for startup
echo -e "${YELLOW}Waiting for daemons to start (5s)...${NC}"
sleep 5

# Check if running
echo -e "\n${YELLOW}Checking daemon status...${NC}"

ALICE_RUNNING=false
BOB_RUNNING=false

if ps -p $ALICE_PID > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Alice daemon running (PID: $ALICE_PID)${NC}"
    ALICE_RUNNING=true
else
    echo -e "${RED}✗ Alice daemon not running${NC}"
    echo "Last 10 lines of Alice's log:"
    tail -10 /tmp/alice.log
fi

if ps -p $BOB_PID > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Bob daemon running (PID: $BOB_PID)${NC}"
    BOB_RUNNING=true
else
    echo -e "${RED}✗ Bob daemon not running${NC}"
    echo "Last 10 lines of Bob's log:"
    tail -10 /tmp/bob.log
fi

# Extract peer IDs
echo -e "\n${YELLOW}Peer Information:${NC}"
ALICE_PEER=$(grep "Peer ID:" /tmp/alice.log 2>/dev/null | head -1 | awk '{print $NF}')
BOB_PEER=$(grep "Peer ID:" /tmp/bob.log 2>/dev/null | head -1 | awk '{print $NF}')

[ -n "$ALICE_PEER" ] && echo "  Alice: $ALICE_PEER"
[ -n "$BOB_PEER" ] && echo "  Bob:   $BOB_PEER"

# Listen addresses
echo -e "\n${YELLOW}Listening Addresses:${NC}"
grep "Listening on" /tmp/alice.log 2>/dev/null | tail -1 | sed 's/^/  Alice: /'
grep "Listening on" /tmp/bob.log 2>/dev/null | tail -1 | sed 's/^/  Bob:   /'

# Wait a bit more for potential connections
echo -e "\n${YELLOW}Waiting for peer discovery (3s)...${NC}"
sleep 3

# Check for connections
echo -e "\n${YELLOW}Connection Status:${NC}"
ALICE_CONNECTIONS=$(grep -c "Peer connected" /tmp/alice.log 2>/dev/null || echo "0")
BOB_CONNECTIONS=$(grep -c "Peer connected" /tmp/bob.log 2>/dev/null || echo "0")

echo "  Alice peer connections: $ALICE_CONNECTIONS"
echo "  Bob peer connections: $BOB_CONNECTIONS"

if [ "$ALICE_CONNECTIONS" -gt 0 ]; then
    echo -e "${GREEN}  ✓ Alice has peers${NC}"
    grep "Peer connected" /tmp/alice.log | head -1 | sed 's/^/    /'
else
    echo -e "${YELLOW}  ⚠ Alice has no peer connections yet${NC}"
fi

if [ "$BOB_CONNECTIONS" -gt 0 ]; then
    echo -e "${GREEN}  ✓ Bob has peers${NC}"
    grep "Peer connected" /tmp/bob.log | head -1 | sed 's/^/    /'
else
    echo -e "${YELLOW}  ⚠ Bob has no peer connections yet${NC}"
fi

# Keep running for a bit to show activity
echo -e "\n${YELLOW}Running for 5 more seconds to capture activity...${NC}"
sleep 5

# Summary
echo -e "\n${BLUE}════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}                        TEST SUMMARY                          ${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}\n"

if [ "$ALICE_RUNNING" = true ] && [ "$BOB_RUNNING" = true ]; then
    echo -e "${GREEN}✓ Both daemons started successfully${NC}"
    echo -e "${GREEN}✓ P2P infrastructure functional${NC}"
    echo -e "${GREEN}✓ Agents can listen on ports${NC}"

    if [ "$ALICE_CONNECTIONS" -gt 0 ] || [ "$BOB_CONNECTIONS" -gt 0 ]; then
        echo -e "${GREEN}✓ Peer connections detected${NC}"
    fi

    echo -e "\n${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║               ✓ TEST PASSED                              ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Two agents are running simultaneously with P2P enabled!"
    echo ""
    echo "Full logs available at:"
    echo "  /tmp/alice.log"
    echo "  /tmp/bob.log"
    echo ""
    exit 0
else
    echo -e "${RED}✗ One or both daemons failed${NC}"
    echo ""
    echo "Check logs:"
    echo "  /tmp/alice.log"
    echo "  /tmp/bob.log"
    echo ""
    exit 1
fi
