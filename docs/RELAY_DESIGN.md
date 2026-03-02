# 🌐 Gork Agent Protocol - Relay Node Design

## Overview

A relay node provides always-on P2P infrastructure for the Gork network, enabling:
- **NAT traversal** for peers behind firewalls
- **Peer discovery** via DHT and relay protocols
- **Message routing** between disconnected peers
- **Network monitoring** and telemetry

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                 Gork Relay Node                              │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              libp2p Stack                              │  │
│  │  • Relay (Circuit Relay) - NAT traversal             │  │
│  │  • Gossipsub - Pub/sub message mesh                  │  │
│  │  • Kademlia DHT - Peer discovery                     │  │
│  │  • Identify - Peer info exchange                    │  │
│  │  • Ping - Liveness checks                            │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │           Gork Agent Logic                            │  │
│  │  • NEAR authentication                               │  │
│  │  • Peer verification                                │  │
│  │  • Message validation                                │  │
│  │  • Reputation tracking                                │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │           Relay Features                             │  │
│  │  • Connection tracking                               │  │
│  │  • Circuit management                                 │  │
│  │  • Peer statistics                                   │  │
│  │  • Rate limiting                                     │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │         Monitoring & API                             │  │
│  │  • Prometheus metrics                                │  │
│  │  • Health check endpoint                              │  │
│  │  • Peer list API                                      │  │
│  │  • Network stats dashboard                           │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. libp2p Relay Implementation

```rust
use libp2p::{
    relay,
    gossipsub, identify, kad, ping,
    swarm::NetworkBehaviour, SwarmBuilder,
};

#[derive(NetworkBehaviour)]
struct RelayBehaviour {
    relay: relay::Behaviour,
    gossipsub: gossipsub::Behaviour,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

impl RelayBehaviour {
    /// Create relay-specific behaviour
    fn new_relay() -> Self {
        // Configure as a circuit relay
        let relay = relay::Behaviour::new(
            relay::Config::new()
                .max_reservations(1024)
                .max_reservations_per_peer(16)
                .reservation_duration(Duration::from_secs(30 * 60))
        );

        // Gossipsub for message propagation
        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(
                libp2p::identity::Keypair::generate_ed25519()
            ),
            gossipsub_config(),
        ).unwrap();

        // Kademlia DHT
        let kademlia = kad::Behaviour::new(peer_id, store);

        // Identify protocol
        let identify = identify::Behaviour::new(identify_config());

        // Ping for liveness
        let ping = ping::Behaviour::new(ping::Config::new());

        Self {
            relay,
            gossipsub,
            kademlia,
            identify,
            ping,
        }
    }
}
```

### 2. Relay Node Structure

```rust
/// Gork Relay Node
pub struct GorkRelay {
    /// P2P swarm
    swarm: Swarm<RelayBehaviour>,

    /// Relay configuration
    config: RelayConfig,

    /// Connected peers tracking
    connected_peers: HashSet<PeerId>,

    /// Active relay circuits
    circuits: HashMap<PeerId, CircuitInfo>,

    /// Statistics
    stats: RelayStats,

    /// NEAR authenticator
    authenticator: Option<PeerAuthenticator>,
}

/// Relay configuration
pub struct RelayConfig {
    /// Listen addresses
    pub listen_addrs: Vec<Multiaddr>,

    /// NEAR account for this relay
    pub near_account: String,

    /// Whether to require peer authentication
    pub require_auth: bool,

    /// Max concurrent relay circuits
    pub max_circuits: usize,

    /// Circuit timeout
    pub circuit_timeout: Duration,

    /// Enable metrics endpoint
    pub enable_metrics: bool,

    /// Metrics port
    pub metrics_port: u16,
}

/// Circuit information
#[derive(Debug, Clone)]
struct CircuitInfo {
    src_peer: PeerId,
    dst_peer: PeerId,
    created_at: Instant,
    bytes_transferred: u64,
}

/// Relay statistics
#[derive(Debug, Clone)]
pub struct RelayStats {
    pub start_time: Instant,
    pub total_connections: u64,
    pub active_circuits: u64,
    pub total_bytes_relayed: u64,
    pub authenticated_peers: u64,
    pub unauthenticated_peers: u64,
}
```

### 3. Relay Configuration Example

```yaml
# relay-config.yaml

# Network configuration
network:
  listen_addrs:
    - /ip4/0.0.0.0/tcp/4001
    - /ip6/::/tcp/4001
  bootstrap_peers:
    - /ip4/1.2.3.4/tcp/4001/p2p/QmPeerId
  mdns: false  # Disable for cloud deployment

# NEAR authentication
near:
  account: "relay.gork-agent.testnet"
  network: "testnet"
  require_peer_auth: true

# Relay settings
relay:
  max_reservations: 10000
  max_reservations_per_peer: 32
  reservation_duration: 30m
  circuit_timeout: 10m
  max_circuits: 5000

# Rate limiting
rate_limit:
  max_circuits_per_peer: 16
  max_bytes_per_second: 10_000_000
  max_messages_per_second: 1000

# Monitoring
monitoring:
  enable_metrics: true
  metrics_port: 9090
  enable_health_check: true
  health_check_port: 8080
  log_level: info

# Topics to subscribe to
topics:
  - "gork-agent-messages"
  - "gork-discovery"
  - "gork-capabilities"
```

## Deployment Scenarios

### Option 1: Public Cloud Relay

**Deployment:** AWS/GCP/DigitalOcean VM

**Requirements:**
- Public IP address
- Open ports: 4001 (P2P), 9090 (metrics), 8080 (health)
- 2+ GB RAM, 1+ CPU
- NEAR credentials for relay account

**Docker Compose:**

```yaml
version: '3.8'

services:
  gork-relay:
    image: gork-agent-relay:latest
    container_name: gork-relay
    restart: unless-stopped
    ports:
      - "4001:4001/tcp"   # P2P
      - "4001:4001/udp"   # P2P QUIC
      - "9090:9090"       # Metrics
      - "8080:8080"       # Health check
    environment:
      - RUST_LOG=info
      - NEAR_NETWORK=testnet
      - NEAR_ACCOUNT=relay.gork-agent.testnet
    volumes:
      - ./relay-config.yaml:/app/config.yaml
      - ~/.near-credentials:/root/.near-credentials:ro
      - relay-data:/app/data
    command: >
      relay
      --config /app/config.yaml
      --account relay.gork-agent.testnet

  prometheus:
    image: prom/prometheus:latest
    container_name: prometheus
    ports:
      - "9091:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'

  grafana:
    image: grafana/grafana:latest
    container_name: grafana
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana-data:/var/lib/grafana

volumes:
  relay-data:
  prometheus-data:
  grafana-data:
```

### Option 2: Residential/Home Relay

**Purpose:** Help peers behind NAT, community contribution

**Requirements:**
- Static IP or port forwarding
- 512MB RAM minimum
- Always-on (RPi, NAS, etc.)

**Systemd Service:**

```ini
# /etc/systemd/system/gork-relay.service

[Unit]
Description=Gork Agent P2P Relay
After=network.target

[Service]
Type=simple
User=gork
WorkingDirectory=/opt/gork-relay
Environment="NEAR_ACCOUNT=relay-user.testnet"
Environment="RUST_LOG=info"
ExecStart=/opt/gork-relay/gork-agent relay daemon
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

### Option 3: Kubernetes Deployment

**Deployment YAML:**

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: gork-relay
  labels:
    app: gork-relay
spec:
  replicas: 3
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
        image: gork-agent-relay:latest
        ports:
        - containerPort: 4001
          name: p2p
          protocol: TCP
        - containerPort: 9090
          name: metrics
          protocol: TCP
        - containerPort: 8080
          name: health
          protocol: TCP
        env:
        - name: NEAR_ACCOUNT
          valueFrom:
            secretKeyRef:
              name: near-credentials
              key: account_id
        - name: NEAR_NETWORK
          value: "mainnet"
        - name: RUST_LOG
          value: "info"
        volumeMounts:
        - name: near-creds
          mountPath: /root/.near-credentials
          readOnly: true
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2000m"
      volumes:
      - name: near-creds
        secret:
          secretName: near-credentials
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
    name: p2p
    targetPort: 4001
  - port: 9090
    name: metrics
    targetPort: 9090
  - port: 8080
    name: health
    targetPort: 8080
  type: LoadBalancer
---
apiVersion: v1
kind: Secret
metadata:
  name: near-credentials
type: Opaque
stringData:
  account_id: relay.gork-agent.testnet
  private_key: ed25519:...
```

## Features

### 1. Circuit Relay Protocol

The relay uses libp2p's built-in circuit relay:

```rust
// When peer A wants to connect to peer B via relay:
// 1. A connects to relay
// 2. A asks relay to connect to B
// 3. Relay creates circuit A -> Relay -> B
// 4. Messages flow through relay

pub async fn handle_relay_request(&mut self, peer: PeerId) {
    if !self.can_create_circuit(&peer) {
        warn!("Rejecting circuit request from {:?}", peer);
        return;
    }

    let circuit = CircuitInfo {
        src_peer: peer,
        dst_peer: /* ... */,
        created_at: Instant::now(),
        bytes_transferred: 0,
    };

    self.circuits.insert(peer, circuit);
    self.stats.active_circuits += 1;

    info!("Created circuit for {:?}", peer);
}
```

### 2. Peer Authentication Enforcement

```rust
// Relay can require NEAR authentication
pub async fn authenticate_peer(&mut self, peer: &PeerId) -> Result<bool> {
    if !self.config.require_auth {
        return Ok(true);  // Allow unauthenticated if not required
    }

    // Send challenge
    let challenge = self.authenticator.create_challenge(peer.to_string());

    // Wait for response with timeout
    let response = tokio::time::timeout(
        Duration::from_secs(30),
        self.wait_for_auth_response(peer)
    ).await??;

    // Verify signature
    match self.authenticator.verify_peer(&response).await {
        Ok(verified) => {
            info!("✅ Peer {:?} verified as {}", peer, verified.near_account);
            self.stats.authenticated_peers += 1;
            Ok(true)
        }
        Err(e) => {
            warn!("❌ Peer {:?} authentication failed: {}", peer, e);
            self.stats.unauthenticated_peers += 1;
            Ok(false)
        }
    }
}
```

### 3. Prometheus Metrics

```rust
use prometheus::{Counter, Histogram, Gauge, Registry};

pub struct RelayMetrics {
    pub connections_total: Counter,
    pub circuits_active: Gauge,
    pub bytes_relayed: Counter,
    pub auth_failures: Counter,
    pub message_latency: Histogram,
}

impl RelayMetrics {
    pub fn new() -> Self {
        Self {
            connections_total: Counter::new(
                "relay_connections_total",
                "Total relay connections"
            ).unwrap(),
            circuits_active: Gauge::new(
                "relay_circuits_active",
                "Currently active circuits"
            ).unwrap(),
            bytes_relayed: Counter::new(
                "relay_bytes_relayed_total",
                "Total bytes relayed"
            ).unwrap(),
            auth_failures: Counter::new(
                "relay_auth_failures_total",
                "Total authentication failures"
            ).unwrap(),
            message_latency: Histogram::new(
                "relay_message_latency_seconds",
                "Message relay latency"
            ).unwrap(),
        }
    }
}

// Expose metrics endpoint
pub async fn metrics_server(metrics: RelayMetrics) -> Result<()> {
    let app = axum::Router::new()
        .route("/metrics", axum::routing::get(metrics_handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], 9090));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
```

### 4. Health Check Endpoint

```rust
pub async fn health_check() -> Json<HealthStatus> {
    Json(HealthStatus {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
        uptime: get_uptime(),
        connected_peers: swarm.connected_peers_count(),
        active_circuits: relay.circuit_count(),
        last_block_height: blockchain.get_height().await,
    })
}

#[derive(Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub version: &'static str,
    pub uptime: u64,
    pub connected_peers: usize,
    pub active_circuits: usize,
    pub last_block_height: u64,
}
```

## CLI Commands

```bash
# Start relay server
gork-agent relay daemon \
  --account relay.gork-agent.testnet \
  --port 4001 \
  --max-circuits 1000 \
  --enable-metrics \
  --metrics-port 9090

# Show relay status
gork-agent relay status

# List connected peers
gork-agent relay peers

# Show circuit statistics
gork-agent relay stats

# Check relay health
curl http://relay.gork-agent.testnet:8080/health

# Get metrics
curl http://relay.gork-agent.testnet:9090/metrics
```

## Example: Full Relay Implementation

```rust
//! Gork Relay Daemon
//!
//! Always-on P2P relay node with NEAR authentication

use gork_agent::{auth::PeerAuthenticator, network::{AgentNetwork, NetworkConfig}};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tokio::select;
use tracing::{info, warn, error};

pub struct RelayNode {
    network: AgentNetwork,
    config: RelayConfig,
    stats: RelayStats,
    circuits: HashMap<PeerId, CircuitInfo>,
}

impl RelayNode {
    pub async fn new(config: RelayConfig) -> Result<Self> {
        // Initialize with NEAR account
        let identity = load_or_create_identity(&config.near_account).await?;
        let authenticator = Some(create_authenticator(&config).await?);

        // Create P2P network with relay enabled
        let network = AgentNetwork::with_auth(
            identity,
            NetworkConfig {
                port: config.listen_addrs[0].port().unwrap_or(4001),
                bootstrap_peers: config.bootstrap_peers.clone(),
            },
            event_sender,
            authenticator,
            config.require_auth,
        ).await?;

        Ok(Self {
            network,
            config,
            stats: RelayStats::default(),
            circuits: HashMap::new(),
        })
    }

    /// Run the relay daemon
    pub async fn run(mut self) -> Result<()> {
        info!("🌐 Starting Gork Relay: {}", self.config.near_account);
        info!("📡 Listening on: {:?}", self.config.listen_addrs);
        info!("🔒 Authentication: {}",
            if self.config.require_auth { "REQUIRED" } else { "OPTIONAL" });

        // Start metrics server if enabled
        if self.config.enable_metrics {
            tokio::spawn(metrics_server(self.config.metrics_port));
        }

        loop {
            select! {
                // Handle P2P events
                event = self.network.swarm.select_next_some() => {
                    self.handle_event(event).await?;
                }

                // Periodic tasks
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    self.maintenance_tasks().await?;
                }

                // Shutdown signal
                _ = tokio::signal::ctrl_c() => {
                    info!("🛑 Shutting down relay");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle P2P swarm event
    async fn handle_event(&mut self, event: SwarmEvent) -> Result<()> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("📡 Listening on {}", address);
            }

            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                info!("✅ Connected: {}", peer_id);
                self.stats.total_connections += 1;
                self.connected_peers.insert(peer_id);
            }

            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                info!("❌ Disconnected: {}", peer_id);
                self.connected_peers.remove(&peer_id);
                self.circuits.remove(&peer_id);
            }

            SwarmEvent::Behaviour(event) => {
                self.handle_behaviour_event(event).await?;
            }

            _ => {}
        }
        Ok(())
    }

    /// Maintenance tasks
    async fn maintenance_tasks(&mut self) -> Result<()> {
        // Clean up expired circuits
        let now = Instant::now();
        let timeout = self.config.circuit_timeout;

        self.circuits.retain(|_, circuit| {
            now.duration_since(circuit.created_at) < timeout
        });

        // Log stats
        info!("📊 Connected peers: {}", self.connected_peers.len());
        info!("🔄 Active circuits: {}", self.circuits.len());
        info!("📈 Total bytes relayed: {}", self.stats.total_bytes_relayed);

        Ok(())
    }
}
```

## Monitoring Stack

### Grafana Dashboard Queries

```promql
# Active relay circuits
relay_circuits_active

# Connections per minute
rate(relay_connections_total[1m])

# Authentication failure rate
rate(relay_auth_failures_total[5m])

# Message relay latency (p95)
histogram_quantile(0.95, rate(relay_message_latency_seconds_bucket[5m]))

# Bytes relayed per hour
rate(relay_bytes_relayed_total[1h]) * 3600
```

### Health Checks

```bash
# Basic health
curl http://relay.gork-agent.testnet:8080/health

# Readiness (only if serving traffic)
curl http://relay.gork-agent.testnet:8080/ready

# Peer list
curl http://relay.gork-agent.testnet:8080/peers

# Circuit info
curl http://relay.gork-agent.testnet:8080/circuits
```

## Cost Estimates

### Cloud VM (DigitalOcean)

**Small Relay:**
- 2 GB RAM, 1 vCPU, 1 TB transfer
- Cost: ~$24/month
- Capacity: ~100 concurrent peers
- Bandwidth: ~1 TB/month

**Large Relay:**
- 8 GB RAM, 4 vCPU, 5 TB transfer
- Cost: ~$96/month
- Capacity: ~500 concurrent peers
- Bandwidth: ~5 TB/month

### Kubernetes (GKE/EKS)

**Small cluster (3 nodes):**
- Cost: ~$150/month
- High availability
- Auto-scaling
- Better resilience

## Best Practices

### 1. **Security**
- ✅ Run as non-root user
- ✅ Use firewall rules
- ✅ Enable authentication
- ✅ Monitor for abuse
- ✅ Rate limiting

### 2. **Reliability**
- ✅ Multiple relays (3+ for HA)
- ✅ Health checks
- ✅ Auto-restart
- ✅ Monitoring alerts
- ✅ Regular backups

### 3. **Performance**
- ✅ Use SSD storage
- ✅ Sufficient bandwidth
- ✅ Connection pooling
- ✅ Metrics tracking
- ✅ Load balancing

### 4. **Operations**
- ✅ Log aggregation
- ✅ Error tracking
- ✅ Performance monitoring
- ✅ Capacity planning
- ✅ Incident response

## Example: Deploy to DigitalOcean

```bash
# 1. Create droplet
doctl compute droplet create \
    --size s-2vcpu-4gb \
    --region nyc1 \
    --image ubuntu-22-04-x64 \
    --ssh-keys ~/.ssh/id_rsa.pub \
    gork-relay-1

# 2. SSH in
ssh root@gork-relay-1

# 3. Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sh get-docker.sh

# 4. Clone repo
git clone https://github.com/your-org/gork-protocol.git
cd gork-protocol

# 5. Start relay
docker-compose up -d

# 6. Verify
curl http://your-droplet-ip:8080/health
```

## Summary

A Gork relay provides:

✅ **NAT traversal** - Connect peers behind firewalls
✅ **Always-on availability** - Stable network entry points
✅ **Peer discovery** - DHT + relay protocols
✅ **Message routing** - Forward between peers
✅ **NEAR authentication** - Verify peer identities
✅ **Monitoring** - Prometheus + Grafana dashboards
✅ **Scalability** - Docker + Kubernetes ready

**Relays are the backbone of a resilient P2P network!** 🌐
