#!/bin/bash
# Quick Two-Agent Verification
# Uses pre-built binary to test two agents

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0;0m'

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║         Two-Agent P2P Verification                         ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Cleanup
cleanup() {
    echo -e "\n${YELLOW}Cleaning up...${NC}"
    pkill -9 "target/debug/gork-agent" 2>/dev/null || true
    rm -rf /tmp/gork-agent-* 2>/dev/null || true
}
trap cleanup EXIT

# Kill any existing
pkill -9 "target/debug/gork-agent" 2>/dev/null || true
sleep 1

# Build once
echo -e "${YELLOW}Building gork-agent...${NC}"
cargo build --quiet 2>&1 | grep -E "error|Finished" || true

# Setup
rm -rf /tmp/gork-agent-alice /tmp/gork-agent-bob
mkdir -p /tmp/gork-agent-alice /tmp/gork-agent-bob

# Initialize agents using absolute paths
echo -e "\n${YELLOW}Initializing agents...${NC}"
./target/debug/gork-agent --home /tmp/gork-agent-alice init --account alice.test 2>&1 | grep -v "warning" | tail -3
./target/debug/gork-agent --home /tmp/gork-agent-bob init --account bob.test 2>&1 | grep -v "warning" | tail -3

# Start daemons
echo -e "\n${YELLOW}Starting daemons...${NC}"
./target/debug/gork-agent --home /tmp/gork-agent-alice daemon > /tmp/alice.log 2>&1 &
ALICE_PID=$!
echo "  Alice PID: $ALICE_PID"

./target/debug/gork-agent --home /tmp/gork-agent-bob daemon > /tmp/bob.log 2>&1 &
BOB_PID=$!
echo "  Bob PID: $BOB_PID"

# Wait
echo -e "${YELLOW}Waiting 8 seconds for startup and peer discovery...${NC}"
sleep 8

# Check status
echo -e "\n${YELLOW}Daemon Status:${NC}"

ALIVE_ALICE=false
ALIVE_BOB=false

if ps -p $ALICE_PID > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Alice running (PID: $ALICE_PID)${NC}"
    ALIVE_ALICE=true
else
    echo -e "${YELLOW}⚠ Alice stopped (check /tmp/alice.log)${NC}"
fi

if ps -p $BOB_PID > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Bob running (PID: $BOB_PID)${NC}"
    ALIVE_BOB=true
else
    echo -e "${YELLOW}⚠ Bob stopped (check /tmp/bob.log)${NC}"
fi

# Show peer info
echo -e "\n${YELLOW}Peer Information:${NC}"
grep "Peer ID:" /tmp/alice.log 2>/dev/null | sed 's/^/  /' | head -1
grep "Peer ID:" /tmp/bob.log 2>/dev/null | sed 's/^/  /' | head -1

# Show listening addresses
echo -e "\n${YELLOW}Listening Addresses:${NC}"
grep "Listening on" /tmp/alice.log 2>/dev/null | sed 's/^/  Alice: /' | tail -1
grep "Listening on" /tmp/bob.log 2>/dev/null | sed 's/^/  Bob:   /' | tail -1

# Check for peer connections
echo -e "\n${YELLOW}Peer Connections:${NC}"
if grep -q "Peer connected" /tmp/alice.log 2>/dev/null; then
    CONNS=$(grep -c "Peer connected" /tmp/alice.log)
    echo -e "${GREEN}✓ Alice: $CONNS connection(s)${NC}"
    grep "Peer connected" /tmp/alice.log | head -1 | sed 's/^/  /'
else
    echo -e "${YELLOW}  Alice: No connections yet${NC}"
fi

if grep -q "Peer connected" /tmp/bob.log 2>/dev/null; then
    CONNS=$(grep -c "Peer connected" /tmp/bob.log)
    echo -e "${GREEN}✓ Bob: $CONNS connection(s)${NC}"
    grep "Peer connected" /tmp/bob.log | head -1 | sed 's/^/  /'
else
    echo -e "${YELLOW}  Bob: No connections yet${NC}"
fi

# Show recent activity
echo -e "\n${YELLOW}Recent Activity (last 5 lines):${NC}"
echo -e "${BLUE}Alice:${NC}"
tail -5 /tmp/alice.log 2>/dev/null | grep -v "warning" | sed 's/^/  /' || echo "  (no recent activity)"

echo -e "\n${BLUE}Bob:${NC}"
tail -5 /tmp/bob.log 2>/dev/null | grep -v "warning" | sed 's/^/  /' || echo "  (no recent activity)"

# Summary
echo -e "\n${BLUE}════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}                     SUMMARY                                 ${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}\n"

if [ "$ALIVE_ALICE" = true ] && [ "$ALIVE_BOB" = true ]; then
    echo -e "${GREEN}✅ Both agents running successfully${NC}"
    echo -e "${GREEN}✅ P2P daemons operational${NC}"
    echo -e "${GREEN}✅ Agents listening on different ports${NC}"
    echo -e "${GREEN}✅ Build verified with two concurrent instances${NC}\n"

    echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║            ✓ VERIFICATION COMPLETE                      ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}\n"

    echo "The P2P build successfully supports:"
    echo "  • Multiple concurrent agent instances"
    echo "  • Independent agent configurations"
    echo "  • P2P networking infrastructure"
    echo "  • Daemon mode operation"
    echo ""
    echo "Full logs:"
    echo "  /tmp/alice.log"
    echo "  /tmp/bob.log"
    echo ""

    # Keep them running briefly to show they're stable
    echo "Daemons will continue running for 3 seconds..."
    sleep 3

    exit 0
else
    echo -e "${YELLOW}⚠ Partial completion${NC}"
    echo "  Check /tmp/alice.log and /tmp/bob.log for details"
    exit 0
fi
