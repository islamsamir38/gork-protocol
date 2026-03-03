//! Power of Two Random Choices (P2C) Load Balancing
//!
//! Implements the P2C algorithm for distributed load balancing without
//! central coordination. Optimal for P2P networks where each node
//! makes independent decisions.
//!
//! Reference: Mitzenmacher, Richa & Sitaraman (2001)
//! "The Power of Two Random Choices: A Survey of Techniques and Results"

use libp2p::PeerId;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Load metrics for a peer
#[derive(Debug, Clone)]
pub struct PeerLoad {
    /// Number of active connections
    pub connection_count: usize,
    /// Number of pending requests
    pub pending_requests: usize,
    /// Last known RTT (for latency-based selection)
    pub last_rtt: Option<Duration>,
    /// Last time this peer was seen
    pub last_seen: Instant,
    /// Total messages forwarded to this peer
    pub messages_forwarded: usize,
}

impl Default for PeerLoad {
    fn default() -> Self {
        Self {
            connection_count: 0,
            pending_requests: 0,
            last_rtt: None,
            last_seen: Instant::now(),
            messages_forwarded: 0,
        }
    }
}

impl PeerLoad {
    /// Calculate a composite load score (lower is better)
    pub fn score(&self) -> usize {
        // Weight connections more heavily than pending requests
        self.connection_count * 10 + self.pending_requests
    }

    /// Calculate latency score (lower is better)
    pub fn latency_score(&self) -> u64 {
        self.last_rtt.map(|d| d.as_millis() as u64).unwrap_or(u64::MAX)
    }

    /// Check if peer is considered healthy
    pub fn is_healthy(&self) -> bool {
        // Consider peer unhealthy if not seen in 5 minutes
        self.last_seen.elapsed() < Duration::from_secs(300)
    }
}

/// Load tracker for all peers
#[derive(Debug, Clone)]
pub struct PeerLoadTracker {
    loads: HashMap<PeerId, PeerLoad>,
}

impl Default for PeerLoadTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerLoadTracker {
    pub fn new() -> Self {
        Self {
            loads: HashMap::new(),
        }
    }

    /// Update load for a peer
    pub fn update(&mut self, peer_id: PeerId, load: PeerLoad) {
        self.loads.insert(peer_id, load);
    }

    /// Get load for a specific peer
    pub fn get(&self, peer_id: &PeerId) -> Option<&PeerLoad> {
        self.loads.get(peer_id)
    }

    /// Get mutable load for a specific peer
    pub fn get_mut(&mut self, peer_id: &PeerId) -> Option<&mut PeerLoad> {
        self.loads.get_mut(peer_id)
    }

    /// Record a connection change
    pub fn record_connection(&mut self, peer_id: PeerId, delta: i32) {
        let load = self.loads.entry(peer_id).or_default();
        load.last_seen = Instant::now();
        if delta > 0 {
            load.connection_count = load.connection_count.saturating_add(delta as usize);
        } else {
            load.connection_count = load.connection_count.saturating_sub((-delta) as usize);
        }
    }

    /// Record a request being sent to peer
    pub fn record_request_start(&mut self, peer_id: PeerId) {
        let load = self.loads.entry(peer_id).or_default();
        load.last_seen = Instant::now();
        load.pending_requests += 1;
    }

    /// Record a request completion
    pub fn record_request_end(&mut self, peer_id: PeerId, rtt: Duration) {
        if let Some(load) = self.loads.get_mut(&peer_id) {
            load.pending_requests = load.pending_requests.saturating_sub(1);
            load.last_rtt = Some(rtt);
            load.last_seen = Instant::now();
        }
    }

    /// Record message forwarded to peer
    pub fn record_forward(&mut self, peer_id: PeerId) {
        let load = self.loads.entry(peer_id).or_default();
        load.messages_forwarded += 1;
        load.last_seen = Instant::now();
    }

    /// Remove stale peers (not seen in threshold)
    pub fn cleanup_stale(&mut self, threshold: Duration) {
        self.loads.retain(|_, load| load.last_seen.elapsed() < threshold);
    }

    /// Get all tracked peers
    pub fn peers(&self) -> impl Iterator<Item = &PeerId> {
        self.loads.keys()
    }

    /// Get healthy peers only
    pub fn healthy_peers(&self) -> Vec<&PeerId> {
        self.loads
            .iter()
            .filter(|(_, load)| load.is_healthy())
            .map(|(peer_id, _)| peer_id)
            .collect()
    }
}

/// P2C Selector - Power of Two Random Choices
pub struct P2CSelector {
    tracker: PeerLoadTracker,
}

impl Default for P2CSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl P2CSelector {
    pub fn new() -> Self {
        Self {
            tracker: PeerLoadTracker::new(),
        }
    }

    /// Get reference to the load tracker for updates
    pub fn tracker(&self) -> &PeerLoadTracker {
        &self.tracker
    }

    /// Get mutable reference to the load tracker
    pub fn tracker_mut(&mut self) -> &mut PeerLoadTracker {
        &mut self.tracker
    }

    /// Select a peer using Power of Two Random Choices
    ///
    /// Algorithm:
    /// 1. Pick 2 random peers from candidates
    /// 2. Choose the one with lower load
    ///
    /// Returns None if no candidates available
    pub fn select_peer(&self, candidates: &[PeerId]) -> Option<PeerId> {
        if candidates.is_empty() {
            return None;
        }
        if candidates.len() == 1 {
            return Some(candidates[0]);
        }

        let mut rng = rand::thread_rng();
        let selected: Vec<_> = candidates.choose_multiple(&mut rng, 2).collect();

        let load1 = self.tracker.get(selected[0]).map(|l| l.score()).unwrap_or(0);
        let load2 = self.tracker.get(selected[1]).map(|l| l.score()).unwrap_or(0);

        Some(if load1 <= load2 {
            *selected[0]
        } else {
            *selected[1]
        })
    }

    /// Select multiple peers for message fanout
    ///
    /// Uses P2C for each selection, excluding previously selected peers
    pub fn select_multiple(&self, candidates: &[PeerId], count: usize) -> Vec<PeerId> {
        if candidates.is_empty() || count == 0 {
            return vec![];
        }

        let mut remaining: Vec<_> = candidates.to_vec();
        let mut selected = Vec::with_capacity(count.min(candidates.len()));

        while selected.len() < count && !remaining.is_empty() {
            if let Some(peer) = self.select_peer(&remaining) {
                selected.push(peer);
                remaining.retain(|p| *p != peer);
            } else {
                break;
            }
        }

        selected
    }

    /// Select peer with lowest latency (for latency-sensitive operations)
    pub fn select_lowest_latency(&self, candidates: &[PeerId]) -> Option<PeerId> {
        if candidates.is_empty() {
            return None;
        }
        if candidates.len() == 1 {
            return Some(candidates[0]);
        }

        let mut rng = rand::thread_rng();
        let selected: Vec<_> = candidates.choose_multiple(&mut rng, 2).collect();

        let lat1 = self.tracker.get(selected[0]).map(|l| l.latency_score()).unwrap_or(u64::MAX);
        let lat2 = self.tracker.get(selected[1]).map(|l| l.latency_score()).unwrap_or(u64::MAX);

        Some(if lat1 <= lat2 {
            *selected[0]
        } else {
            *selected[1]
        })
    }

    /// Select peer with least messages forwarded (for fairness)
    pub fn select_least_used(&self, candidates: &[PeerId]) -> Option<PeerId> {
        if candidates.is_empty() {
            return None;
        }
        if candidates.len() == 1 {
            return Some(candidates[0]);
        }

        let mut rng = rand::thread_rng();
        let selected: Vec<_> = candidates.choose_multiple(&mut rng, 2).collect();

        let fwd1 = self.tracker.get(selected[0]).map(|l| l.messages_forwarded).unwrap_or(0);
        let fwd2 = self.tracker.get(selected[1]).map(|l| l.messages_forwarded).unwrap_or(0);

        Some(if fwd1 <= fwd2 {
            *selected[0]
        } else {
            *selected[1]
        })
    }
}

/// Relay selector for clients choosing among multiple relays
#[derive(Debug, Clone)]
pub struct RelayInfo {
    pub peer_id: PeerId,
    pub addr: libp2p::Multiaddr,
    pub active_circuits: usize,
    pub latency: Option<Duration>,
    pub last_success: Option<Instant>,
}

impl RelayInfo {
    /// Calculate relay score (lower is better)
    pub fn score(&self) -> u64 {
        let base = self.active_circuits as u64 * 10;
        let latency_penalty = self.latency.map(|d| d.as_millis() as u64).unwrap_or(1000);
        let stale_penalty = self.last_success.map(|t| {
            if t.elapsed() > Duration::from_secs(300) { 100 } else { 0 }
        }).unwrap_or(100);

        base + latency_penalty + stale_penalty
    }
}

/// P2C selector for relays
pub struct RelaySelector {
    relays: Vec<RelayInfo>,
}

impl Default for RelaySelector {
    fn default() -> Self {
        Self::new()
    }
}

impl RelaySelector {
    pub fn new() -> Self {
        Self {
            relays: Vec::new(),
        }
    }

    /// Add or update a relay
    pub fn add_relay(&mut self, info: RelayInfo) {
        if let Some(existing) = self.relays.iter_mut().find(|r| r.peer_id == info.peer_id) {
            *existing = info;
        } else {
            self.relays.push(info);
        }
    }

    /// Remove a relay
    pub fn remove_relay(&mut self, peer_id: &PeerId) {
        self.relays.retain(|r| r.peer_id != *peer_id);
    }

    /// Get all known relays
    pub fn relays(&self) -> &[RelayInfo] {
        &self.relays
    }

    /// Select best relay using P2C
    pub fn select_relay(&self) -> Option<&RelayInfo> {
        if self.relays.is_empty() {
            return None;
        }
        if self.relays.len() == 1 {
            return Some(&self.relays[0]);
        }

        let mut rng = rand::thread_rng();
        let selected: Vec<_> = self.relays.choose_multiple(&mut rng, 2).collect();

        let score1 = selected[0].score();
        let score2 = selected[1].score();

        Some(if score1 <= score2 {
            selected[0]
        } else {
            selected[1]
        })
    }

    /// Get relay address for connection
    pub fn get_relay_addr(&self) -> Option<libp2p::Multiaddr> {
        self.select_relay().map(|r| r.addr.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peer_id(n: u8) -> PeerId {
        // Use libp2p's identity to create valid peer IDs
        use libp2p::identity;
        let mut bytes = [0u8; 32];
        bytes[0] = n;
        bytes[31] = n;
        let secret = identity::ed25519::SecretKey::try_from_bytes(bytes).unwrap();
        let keypair = identity::Keypair::from(identity::ed25519::Keypair::from(secret));
        keypair.public().to_peer_id()
    }

    #[test]
    fn test_p2c_select_single_peer() {
        let selector = P2CSelector::new();
        let peer = make_peer_id(1);
        let candidates = vec![peer];

        let selected = selector.select_peer(&candidates);
        assert_eq!(selected, Some(peer));
    }

    #[test]
    fn test_p2c_select_empty() {
        let selector = P2CSelector::new();
        let candidates: Vec<PeerId> = vec![];

        let selected = selector.select_peer(&candidates);
        assert_eq!(selected, None);
    }

    #[test]
    fn test_p2c_selects_less_loaded() {
        let mut selector = P2CSelector::new();
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        // Set peer1 as heavily loaded
        selector.tracker_mut().update(peer1, PeerLoad {
            connection_count: 100,
            pending_requests: 50,
            ..Default::default()
        });

        // Set peer2 as lightly loaded
        selector.tracker_mut().update(peer2, PeerLoad {
            connection_count: 1,
            pending_requests: 0,
            ..Default::default()
        });

        let candidates = vec![peer1, peer2];

        // Run selection many times - should heavily favor peer2
        let mut peer2_selected = 0;
        for _ in 0..1000 {
            if selector.select_peer(&candidates) == Some(peer2) {
                peer2_selected += 1;
            }
        }

        // P2C should select peer2 significantly more often
        // (not 100% because there's randomness, but >80%)
        assert!(peer2_selected > 800);
    }

    #[test]
    fn test_peer_load_score() {
        let load = PeerLoad {
            connection_count: 5,
            pending_requests: 3,
            ..Default::default()
        };
        // 5 * 10 + 3 = 53
        assert_eq!(load.score(), 53);
    }

    #[test]
    fn test_relay_selector() {
        let mut selector = RelaySelector::new();
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        selector.add_relay(RelayInfo {
            peer_id: peer1,
            addr: "/ip4/127.0.0.1/tcp/4001".parse().unwrap(),
            active_circuits: 100,
            latency: Some(Duration::from_millis(10)),
            last_success: Some(Instant::now()),
        });

        selector.add_relay(RelayInfo {
            peer_id: peer2,
            addr: "/ip4/127.0.0.1/tcp/4002".parse().unwrap(),
            active_circuits: 1,
            latency: Some(Duration::from_millis(5)),
            last_success: Some(Instant::now()),
        });

        // Should favor peer2 (lower circuits, lower latency)
        let mut peer2_selected = 0;
        for _ in 0..1000 {
            if selector.select_relay().map(|r| r.peer_id) == Some(peer2) {
                peer2_selected += 1;
            }
        }

        assert!(peer2_selected > 800);
    }

    #[test]
    fn test_select_multiple() {
        let selector = P2CSelector::new();
        let peers: Vec<PeerId> = (0..10).map(|i| make_peer_id(i)).collect();

        let selected = selector.select_multiple(&peers, 3);
        assert_eq!(selected.len(), 3);
        assert_eq!(selected.len(), selected.iter().collect::<std::collections::HashSet<_>>().len());
    }
}
