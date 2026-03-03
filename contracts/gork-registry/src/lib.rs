//! Gork Registry Contract - Trust-ranked relay registry for decentralized P2P network

use near_sdk::{
    collections::{LookupMap, Vector},
    env, near_bindgen,
    serde::{Deserialize, Serialize},
    AccountId, BorshStorageKey, PanicOnDefault, Promise, NearToken,
};

// Constants
const ONE_NEAR: u128 = 1_000_000_000_000_000_000_000_000;
const MIN_STAKE: u128 = 10 * ONE_NEAR;
const MIN_TRUST_TO_JOIN: f64 = 25.0;
const HEARTBEAT_INTERVAL_NS: u64 = 24 * 60 * 60 * 1_000_000_000;
const TIER_PRIMARY: f64 = 80.0;
const TIER_STANDARD: f64 = 60.0;
const TIER_BACKUP: f64 = 40.0;

// Relay tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum RelayTier {
    Primary,
    Standard,
    Backup,
    Candidate,
}

impl RelayTier {
    fn from_score(score: f64) -> Self {
        if score >= TIER_PRIMARY { Self::Primary }
        else if score >= TIER_STANDARD { Self::Standard }
        else if score >= TIER_BACKUP { Self::Backup }
        else { Self::Candidate }
    }
}

// Relay info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct RelayInfo {
    pub account_id: AccountId,
    pub dns_contract: AccountId,
    pub peer_id: String,
    pub stake: u128,
    pub trust_score: f64,
    pub uptime_percent: f64,
    pub jobs_completed: u64,
    pub jobs_successful: u64,
    pub tier: RelayTier,
    pub registered_at: u64,
    pub last_heartbeat: u64,
}

impl RelayInfo {
    fn calculate_rank(&self) -> f64 {
        let stake_score = if self.stake > 0 { (self.stake as f64 / ONE_NEAR as f64).ln() / 10.0 } else { 0.0 };
        let trust = self.trust_score / 100.0;
        let uptime = self.uptime_percent / 100.0;
        let jobs = if self.jobs_completed > 0 { (self.jobs_completed as f64).ln() / 10.0 } else { 0.0 };
        (stake_score * 30.0 + trust * 40.0 + uptime * 20.0 + jobs * 10.0).min(100.0)
    }

    fn update_tier(&mut self) {
        self.tier = RelayTier::from_score(self.calculate_rank());
    }

    fn is_active(&self) -> bool {
        env::block_timestamp() - self.last_heartbeat < HEARTBEAT_INTERVAL_NS
    }
}

// Network stats
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct NetworkStats {
    pub total_relays: u64,
    pub active_relays: u64,
    pub total_stake: String,
    pub primary_count: u64,
    pub standard_count: u64,
    pub backup_count: u64,
    pub candidate_count: u64,
}

// Storage keys
#[derive(BorshStorageKey)]
enum StorageKey {
    Relays,
    RelayList,
}

// Contract
#[near_bindgen]
#[derive(PanicOnDefault)]
pub struct GorkRegistry {
    relays: LookupMap<AccountId, RelayInfo>,
    relay_list: Vector<AccountId>,
    owner: AccountId,
    min_stake: u128,
    reward_pool: u128,
}

#[near_bindgen]
impl GorkRegistry {
    #[init]
    pub fn new(owner: AccountId) -> Self {
        Self {
            relays: LookupMap::new(StorageKey::Relays),
            relay_list: Vector::new(StorageKey::RelayList),
            owner,
            min_stake: MIN_STAKE,
            reward_pool: 0,
        }
    }

    #[payable]
    pub fn register_relay(&mut self, dns_contract: AccountId, peer_id: String, trust_score: f64) -> bool {
        let account_id = env::signer_account_id();
        let stake = env::attached_deposit().as_yoctonear();

        if stake < self.min_stake {
            env::panic_str("Insufficient stake");
        }
        if trust_score < MIN_TRUST_TO_JOIN {
            env::panic_str("Insufficient trust score");
        }
        if !peer_id.starts_with("12D3Koo") {
            env::panic_str("Invalid peer ID");
        }
        if self.relays.contains_key(&account_id) {
            env::panic_str("Already registered");
        }

        let now = env::block_timestamp();
        let mut relay = RelayInfo {
            account_id: account_id.clone(),
            dns_contract,
            peer_id,
            stake,
            trust_score,
            uptime_percent: 100.0,
            jobs_completed: 0,
            jobs_successful: 0,
            tier: RelayTier::Candidate,
            registered_at: now,
            last_heartbeat: now,
        };
        relay.update_tier();

        self.relays.insert(&account_id, &relay);
        self.relay_list.push(&account_id);
        true
    }

    pub fn heartbeat(&mut self) -> bool {
        let account_id = env::signer_account_id();
        let mut relay = self.relays.get(&account_id).expect("Not registered");
        relay.last_heartbeat = env::block_timestamp();
        relay.update_tier();
        self.relays.insert(&account_id, &relay);
        true
    }

    pub fn get_bootstrap_peers(&self, count: u32) -> Vec<RelayInfo> {
        let mut peers: Vec<RelayInfo> = self.relay_list.iter()
            .filter_map(|id| self.relays.get(&id))
            .filter(|r| r.is_active())
            .collect();
        
        peers.sort_by(|a, b| {
            b.calculate_rank().partial_cmp(&a.calculate_rank()).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        peers.into_iter().take(count as usize).collect()
    }

    pub fn get_relay(&self, account_id: AccountId) -> Option<RelayInfo> {
        self.relays.get(&account_id)
    }

    pub fn get_all_relays(&self) -> Vec<RelayInfo> {
        self.relay_list.iter()
            .filter_map(|id| self.relays.get(&id))
            .collect()
    }

    pub fn get_network_stats(&self) -> NetworkStats {
        let relays: Vec<RelayInfo> = self.relay_list.iter()
            .filter_map(|id| self.relays.get(&id))
            .collect();

        let mut stats = NetworkStats {
            total_relays: relays.len() as u64,
            active_relays: 0,
            total_stake: "0".to_string(),
            primary_count: 0,
            standard_count: 0,
            backup_count: 0,
            candidate_count: 0,
        };

        let mut total: u128 = 0;
        for r in &relays {
            if r.is_active() { stats.active_relays += 1; }
            total += r.stake;
            match r.tier {
                RelayTier::Primary => stats.primary_count += 1,
                RelayTier::Standard => stats.standard_count += 1,
                RelayTier::Backup => stats.backup_count += 1,
                RelayTier::Candidate => stats.candidate_count += 1,
            }
        }
        stats.total_stake = format!("{} NEAR", total / ONE_NEAR);
        stats
    }

    pub fn withdraw(&mut self) {
        let account_id = env::signer_account_id();
        let relay = self.relays.get(&account_id).expect("Not registered");
        let stake = relay.stake;

        self.relays.remove(&account_id);
        
        let old_list: Vec<AccountId> = self.relay_list.iter().collect();
        self.relay_list = Vector::new(StorageKey::RelayList);
        for id in old_list {
            if id != account_id {
                self.relay_list.push(&id);
            }
        }

        Promise::new(account_id).transfer(NearToken::from_yoctonear(stake));
    }
}
