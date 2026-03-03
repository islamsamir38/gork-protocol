#!/bin/bash
set -e

# Initialize agent if not already initialized
if [ ! -f /home/gork/.gork-agent/LOCK ]; then
    echo "Initializing relay agent..."
    gork-agent init --account relay.gork.protocol --dev-mode
fi

# Start relay
exec gork-agent relay --port 4001 --max-circuits 1000 --metrics --metrics-port 9090
