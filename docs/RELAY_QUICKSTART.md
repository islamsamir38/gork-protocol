# 🚀 Minimal Gork Relay - Quick Start

A minimal, working P2P relay node for the Gork Agent Protocol.

## What It Does

The relay helps peers behind NAT/firewalls connect to each other:
- ✅ Provides stable P2P entry point
- ✅ Enables NAT traversal
- ✅ Routes messages between peers
- ✅ Runs in Docker or bare metal
- ✅ Optional metrics endpoint

## Quick Start

### 1. Initialize Agent (One Time)

```bash
# For development/testing
gork-agent init --account relay.testnet --dev-mode

# For production (requires NEAR CLI)
near login --account-id relay.gork-agent.testnet
gork-agent init --account relay.gork-agent.testnet
```

### 2. Start Relay

```bash
# Basic relay
gork-agent relay

# Custom port
gork-agent relay --port 5000

# With metrics
gork-agent relay --metrics

# Full options
gork-agent relay \
  --port 4001 \
  --max-circuits 1000 \
  --metrics \
  --metrics-port 9090
```

### 3. Connect Peers to Relay

When starting your agent, specify the relay as a bootstrap peer:

```bash
gork-agent daemon --bootstrap-peers /ip4/<RELAY-IP>/tcp/4001/p2p/<RELAY-PEER-ID>
```

The relay's Peer ID will be shown when you start it.

## Docker Deployment

### Build and Run

```bash
# Build
docker build -f Dockerfile.relay -t gork-relay .

# Run
docker run -d \
  --name gork-relay \
  -p 4001:4001 \
  -p 9090:9090 \
  -v ~/.near-credentials:/home/gork/.near-credentials:ro \
  gork-relay
```

### Docker Compose

```bash
# Start relay
docker-compose -f docker-compose.relay.yml up -d

# View logs
docker-compose -f docker-compose.relay.yml logs -f

# Stop relay
docker-compose -f docker-compose.relay.yml down
```

## Cloud Deployment

### DigitalOcean Droplet

```bash
# 1. Create droplet (Ubuntu 22.04)
doctl compute droplet create gork-relay \
  --size s-1vcpu-1gb \
  --region nyc1 \
  --image ubuntu-22-04-x64

# 2. SSH in
ssh root@<droplet-ip>

# 3. Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sh get-docker.sh

# 4. Run relay
docker run -d \
  --name gork-relay \
  --restart unless-stopped \
  -p 4001:4001 \
  -p 9090:9090 \
  gork-relay:latest
```

### Kubernetes

```yaml
# relay-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: gork-relay
spec:
  replicas: 1
  selector:
    matchLabels:
      app: gork-relay
  template:
    metadata:
      labels:
        app: gork-relay
    spec:
      containers:
      - name: relay
        image: gork-relay:latest
        ports:
        - containerPort: 4001
        - containerPort: 9090
        env:
        - name: RUST_LOG
          value: "info"
---
apiVersion: v1
kind: Service
metadata:
  name: gork-relay
spec:
  selector:
    app: gork-relay
  ports:
  - port: 4001
    targetPort: 4001
    name: p2p
  - port: 9090
    targetPort: 9090
    name: metrics
  type: LoadBalancer
```

## Checking Relay Status

### Health Check

```bash
curl http://localhost:9090/health
# Response: {"status":"healthy","relay":"gork-agent-relay"}
```

### Metrics

```bash
curl http://localhost:9090/metrics
# Response:
# HELP gork_relay_up Relay is running
# TYPE gork_relay_up gauge
# gork_relay_up 1
# HELP gork_relay_peer_id Relay peer ID
# TYPE gork_relay_peer_id gauge
# gork_relay_peer_id "12D3KooW..."
```

### View Logs

```bash
# Docker
docker logs -f gork-relay

# Docker Compose
docker-compose -f docker-compose.relay.yml logs -f

# Bare metal
journalctl -u gork-relay -f
```

## Production Checklist

- [ ] Initialize with NEAR verification (not dev-mode)
- [ ] Use static IP or configure DNS
- [ ] Open ports in firewall (4001, 9090)
- [ ] Set up monitoring
- [ ] Configure log aggregation
- [ ] Enable health checks
- [ ] Set resource limits
- [ ] Configure auto-restart
- [ ] Backup credentials
- [ ] Document relay addresses

## Cost Estimate

**VPS Requirements:**
- 1 GB RAM minimum
- 1 vCPU
- 1 TB bandwidth/month

**Estimated Cost:**
- DigitalOcean: ~$6-12/month
- AWS Lightsail: ~$3.50-5/month
- Vultr: ~$2.40-6/month
- Home/Raspberry Pi: Free (after hardware cost)

## Troubleshooting

### Relay Not Starting

```bash
# Check if agent is initialized
gork-agent whoami

# If not, initialize first
gork-agent init --account relay.testnet --dev-mode
```

### Peers Can't Connect

```bash
# Check firewall
sudo ufw status
sudo ufw allow 4001/tcp

# Check relay is listening
netstat -tuln | grep 4001

# Get relay Peer ID
gork-agent whoami
# Or check logs when relay starts
```

### High Memory Usage

```bash
# Check resource usage
docker stats gork-relay

# Limit memory
docker run -d \
  --memory="512m" \
  --memory-swap="1g" \
  gork-relay
```

## Example Output

```
🌐 Gork Agent Relay Server
============================================================

🤖 Relay Identity: relay.testnet
📡 Port: 4001
📊 Metrics: http://0.0.0.0:9090

✅ Relay listening on: /ip4/0.0.0.0/tcp/4001
   Peer ID: 12D3KooWF...

📝 For peers to connect via this relay:
   Use: --bootstrap-peers /ip4/<YOUR-IP>/tcp/4001/p2p/12D3KooWF...

✅ Relay started successfully!
   Press Ctrl+C to stop
```

## Next Steps

1. **Deploy multiple relays** for redundancy
2. **Monitor metrics** with Prometheus/Grafana
3. **Set up alerts** for relay downtime
4. **Document your relay addresses** for peers
5. **Join the relay network** - share your addresses!

## Support

- Issues: https://github.com/your-org/gork-protocol/issues
- Docs: See RELAY_DESIGN.md for full architecture

---

**Need help?** Start the relay with `--dev-mode` for testing!
