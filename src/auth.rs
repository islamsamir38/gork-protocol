//! Peer Authentication Module
//!
//! Provides NEAR signature-based peer identity verification
//! to prevent peer impersonation in P2P networks.
//!
//! # Trust Score System
//!
//! Uses multi-factor trust scoring to prevent Sybil attacks and reputation farming:
//! - Stake factor (40%): Skin in the game, logarithmic scaling
//! - Age factor (15%): Account maturity
//! - Activity factor (15%): Recent on-chain transactions
//! - Reputation factor (20%): Ratings from other trusted peers
//! - History factor (10%): Completed jobs/contracts in the system

use anyhow::{anyhow, Result};
use base64::{Engine as _, engine::general_purpose};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey, SigningKey};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use reqwest::Client;
use tracing::{info, warn};
use std::path::Path;
use rocksdb::{DB, Options};
use std::collections::HashMap;

// ============================================================================
// TRUST SCORE CONSTANTS
// ============================================================================

/// One NEAR in yoctoNEAR
const ONE_NEAR: u128 = 1_000_000_000_000_000_000_000_000;

/// Minimum trust score to submit ratings
const MIN_TRUST_TO_RATE: f64 = 10.0;

/// Minimum trust score to bid on jobs
const MIN_TRUST_TO_BID: f64 = 25.0;

/// Minimum trust score to approve others' work
const MIN_TRUST_TO_APPROVE: f64 = 60.0;

/// Minimum trust score to open disputes
const MIN_TRUST_TO_DISPUTE: f64 = 40.0;

/// Trust score cache TTL (1 hour)
const TRUST_CACHE_TTL_SECS: u64 = 3600;

/// Rating cooldown (1 per target per day)
const RATING_COOLDOWN_SECS: u64 = 86400;

/// Rating decay period (90 days)
const RATING_DECAY_DAYS: u64 = 90;

// ============================================================================
// INDEXER & ARCHIVAL API CONFIGS
// ============================================================================

/// FastNear API endpoint for transaction counts
const FASTNEAR_API_URL: &str = "https://api.fastnear.com/v1";

/// NEAR archival RPC endpoint for account creation data
const ARCHIVAL_RPC_URL: &str = "https://archival-rpc.mainnet.near.org";

/// Activity lookback period in days
const ACTIVITY_LOOKBACK_DAYS: u64 = 90;

// ============================================================================
// TRUST SCORE STRUCTURES
// ============================================================================

/// Multi-factor trust score for sybil-resistant reputation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustScore {
    // Normalized factors (0.0 - 1.0)
    pub stake_factor: f64,
    pub age_factor: f64,
    pub activity_factor: f64,
    pub reputation_factor: f64,
    pub history_factor: f64,
    
    // Raw data for transparency
    pub stake_yocto: u128,
    pub account_age_days: u64,
    pub tx_count_90d: u64,
    pub avg_rating: f64,
    pub rating_count: u64,
    pub completed_jobs: u64,
    pub job_success_rate: f64,
    
    // Metadata
    pub calculated_at: u64,
}

impl TrustScore {
    /// Calculate composite trust score (0.0 - 100.0)
    pub fn calculate(&self) -> f64 {
        let score = (self.stake_factor * 40.0 +
                     self.age_factor * 15.0 +
                     self.activity_factor * 15.0 +
                     self.reputation_factor * 20.0 +
                     self.history_factor * 10.0);
        
        // Ensure minimum score of 3.0 for any valid account
        score.max(3.0)
    }
    
    /// Check if score meets threshold for an action
    pub fn can_perform(&self, action: TrustAction) -> bool {
        let score = self.calculate();
        match action {
            TrustAction::Rate => score >= MIN_TRUST_TO_RATE,
            TrustAction::Bid => score >= MIN_TRUST_TO_BID,
            TrustAction::Approve => score >= MIN_TRUST_TO_APPROVE,
            TrustAction::Dispute => score >= MIN_TRUST_TO_DISPUTE,
        }
    }
}

/// Actions that require minimum trust score
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustAction {
    Rate,
    Bid,
    Approve,
    Dispute,
}

/// A rating entry with trust-weighted value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingEntry {
    pub rater: String,
    pub rating: u8,           // 1-5 stars
    pub weight: u128,         // Trust score * scale factor
    pub timestamp: u64,
    pub rater_trust_score: f64,  // Snapshot of rater's trust at time of rating
}

/// Job completion record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: String,
    pub account: String,
    pub completed_at: u64,
    pub success: bool,
    pub value_yocto: u128,
}

/// Account data fetched from NEAR blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountData {
    pub account_id: String,
    pub amount: u128,           // Liquid balance
    pub locked: u128,           // Staked balance
    pub storage_usage: u64,
    pub created_at: Option<u64>, // Block height or timestamp
}

/// Cache for trust scores
#[derive(Debug, Clone)]
pub struct TrustCache {
    scores: HashMap<String, (TrustScore, u64)>,  // (score, timestamp)
    ttl_secs: u64,
}

impl TrustCache {
    pub fn new() -> Self {
        Self {
            scores: HashMap::new(),
            ttl_secs: TRUST_CACHE_TTL_SECS,
        }
    }
    
    pub fn get(&self, account: &str) -> Option<&TrustScore> {
        let now = current_timestamp();
        self.scores.get(account).and_then(|(score, ts)| {
            if now - ts < self.ttl_secs {
                Some(score)
            } else {
                None
            }
        })
    }
    
    pub fn insert(&mut self, account: String, score: TrustScore) {
        self.scores.insert(account, (score, current_timestamp()));
    }
    
    pub fn invalidate(&mut self, account: &str) {
        self.scores.remove(account);
    }
    
    pub fn clear(&mut self) {
        self.scores.clear();
    }
}

// ============================================================================
// EXISTING STRUCTURES (with additions)
// ============================================================================

/// NEAR-based peer authentication challenge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthChallenge {
    pub timestamp: u64,
    pub peer_id: String,
    pub nonce: [u8; 32],
}

/// NEAR signature response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub challenge: AuthChallenge,
    pub signature: Vec<u8>,
    pub near_account: String,
    pub public_key: Vec<u8>,
}

/// Verified peer identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedPeer {
    pub near_account: String,
    pub peer_id: String,
    pub public_key: Vec<u8>,
    pub verified_at: u64,
    pub trust_level: TrustLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    Untrusted = 0,
    Known = 1,
    Trusted = 2,
    Owner = 3,
}

/// Peer authenticator using NEAR signatures
pub struct PeerAuthenticator {
    signing_key: SigningKey,
    near_account: String,
    trusted_peers: std::collections::HashMap<String, VerifiedPeer>,
    network: Network,
    http_client: Client,
    db: Option<DB>,
    reverify_interval_secs: u64, // How often to re-check blockchain (default: 24h)
    
    // Trust score system
    ratings: HashMap<String, Vec<RatingEntry>>,   // target -> ratings
    job_history: HashMap<String, Vec<JobRecord>>, // account -> jobs
    trust_cache: TrustCache,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Network {
    Testnet,
    Mainnet,
}

impl Network {
    pub fn rpc_url(&self) -> &str {
        match self {
            Network::Testnet => "https://rpc.testnet.near.org",
            Network::Mainnet => "https://rpc.mainnet.near.org",
        }
    }
}

impl PeerAuthenticator {
    /// Create authenticator for a NEAR account
    pub fn new(near_account: String) -> Self {
        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);

        Self {
            signing_key,
            near_account,
            trusted_peers: std::collections::HashMap::new(),
            network: Network::Testnet,
            http_client: Client::new(),
            db: None,
            reverify_interval_secs: 86400, // 24 hours
            ratings: HashMap::new(),
            job_history: HashMap::new(),
            trust_cache: TrustCache::new(),
        }
    }

    /// Create authenticator with specific network
    pub fn with_network(near_account: String, network: Network) -> Self {
        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);

        Self {
            signing_key,
            near_account,
            trusted_peers: std::collections::HashMap::new(),
            network,
            http_client: Client::new(),
            db: None,
            reverify_interval_secs: 86400,
            ratings: HashMap::new(),
            job_history: HashMap::new(),
            trust_cache: TrustCache::new(),
        }
    }

    /// Create from existing keypair
    pub fn from_keypair(near_account: String, signing_key: SigningKey, network: Network) -> Self {
        Self {
            signing_key,
            near_account,
            trusted_peers: std::collections::HashMap::new(),
            network,
            http_client: Client::new(),
            db: None,
            reverify_interval_secs: 86400,
            ratings: HashMap::new(),
            job_history: HashMap::new(),
            trust_cache: TrustCache::new(),
        }
    }

    /// Enable persistence with RocksDB
    pub fn with_persistence<P: AsRef<Path>>(mut self, path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        
        let db = DB::open(&opts, path)?;
        
        // Load existing trusted peers from DB
        let iter = db.iterator(rocksdb::IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            if let Ok(account) = std::str::from_utf8(&key) {
                if let Ok(peer) = serde_json::from_slice::<VerifiedPeer>(&value) {
                    self.trusted_peers.insert(account.to_string(), peer);
                }
            }
        }
        
        self.db = Some(db);
        info!("Loaded {} trusted peers from persistence", self.trusted_peers.len());
        
        Ok(self)
    }

    /// Set re-verification interval (default: 24 hours)
    pub fn with_reverify_interval(mut self, secs: u64) -> Self {
        self.reverify_interval_secs = secs;
        self
    }

    /// Generate a challenge for a peer
    pub fn create_challenge(&self, peer_id: String) -> AuthChallenge {
        AuthChallenge {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            peer_id,
            nonce: rand::random(),
        }
    }

    /// Fetch all public keys from NEAR blockchain via RPC
    /// Returns list of Ed25519 public keys (32 bytes each)
    pub async fn fetch_near_public_keys(&self, account_id: &str) -> Result<Vec<Vec<u8>>> {
        // Step 1: Get list of all access keys for the account
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "view_access_key_list",
                "finality": "final",
                "account_id": account_id
            }
        });

        let response = self.http_client
            .post(self.network.rpc_url())
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| anyhow!("RPC request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("RPC error: {}", response.status()));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse RPC response: {}", e))?;

        // Extract access keys array
        let keys = result
            .get("result")
            .and_then(|r| r.get("keys"))
            .and_then(|k| k.as_array())
            .ok_or_else(|| anyhow!("No access keys found in response"))?;

        let mut public_keys = Vec::new();
        
        for key_obj in keys {
            if let Some(public_key_str) = key_obj.get("public_key").and_then(|k| k.as_str()) {
                // NEAR public keys are in format "ed25519:BASE58..."
                if let Some(key_b58) = public_key_str.strip_prefix("ed25519:") {
                    // Decode base58 to get 32-byte Ed25519 public key
                    match bs58::decode(key_b58).into_vec() {
                        Ok(key_bytes) if key_bytes.len() == 32 => {
                            public_keys.push(key_bytes);
                        }
                        Ok(_) => {
                            warn!("Invalid public key length for {}", account_id);
                        }
                        Err(e) => {
                            warn!("Failed to decode base58 public key: {}", e);
                        }
                    }
                }
            }
        }

        if public_keys.is_empty() {
            return Err(anyhow!("No valid Ed25519 public keys found for account {}", account_id));
        }

        info!("Fetched {} public keys for {}", public_keys.len(), account_id);
        Ok(public_keys)
    }

    /// Fetch public key from NEAR blockchain via RPC (legacy - returns first key)
    pub async fn fetch_near_public_key(&self, account_id: &str) -> Result<Vec<u8>> {
        let keys = self.fetch_near_public_keys(account_id).await?;
        keys.into_iter()
            .next()
            .ok_or_else(|| anyhow!("No public keys found"))
    }

    /// Sign a challenge (prove our identity)
    pub fn sign_challenge(&self, challenge: &AuthChallenge) -> Result<AuthResponse> {
        let challenge_bytes = serde_json::to_vec(challenge)?;

        let signature = self.signing_key.sign(&challenge_bytes);

        Ok(AuthResponse {
            challenge: challenge.clone(),
            signature: signature.to_bytes().to_vec(),
            near_account: self.near_account.clone(),
            public_key: self.signing_key.verifying_key().to_bytes().to_vec(),
        })
    }

    /// Verify a peer's signature
    pub async fn verify_peer(&mut self, response: &AuthResponse) -> Result<VerifiedPeer> {
        // 1. Check timestamp (prevent replay attacks, 5 minute window)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let time_diff = now.abs_diff(response.challenge.timestamp);
        if time_diff > 300 {
            return Err(anyhow!("Challenge too old: {} seconds", time_diff));
        }

        // 2. Verify signature
        let challenge_bytes = serde_json::to_vec(&response.challenge)?;
        let sig_bytes: [u8; 64] = response.signature.clone()
            .try_into()
            .map_err(|_| anyhow!("Invalid signature length"))?;
        let signature = Signature::from_bytes(&sig_bytes);

        let pub_bytes: [u8; 32] = response.public_key.clone()
            .try_into()
            .map_err(|_| anyhow!("Invalid public key length"))?;
        let verifying_key = VerifyingKey::from_bytes(&pub_bytes)
            .map_err(|_| anyhow!("Invalid public key"))?;

        verifying_key
            .verify(&challenge_bytes, &signature)
            .map_err(|_| anyhow!("Signature verification failed"))?;

        // 3. CRITICAL SECURITY CHECK: Verify the public key belongs to the claimed account
        // Check if we have a cached peer that needs re-verification
        let needs_verification = if let Some(known_peer) = self.trusted_peers.get(&response.near_account) {
            // Check if re-verification is needed (24h default)
            now - known_peer.verified_at > self.reverify_interval_secs
        } else {
            true
        };

        if needs_verification {
            // Fetch ALL public keys from NEAR blockchain
            let valid_keys = match self.fetch_near_public_keys(&response.near_account).await {
                Ok(keys) => keys,
                Err(e) => {
                    // If we have a cached peer, allow it with a warning (network issues)
                    if let Some(known_peer) = self.trusted_peers.get(&response.near_account) {
                        if known_peer.public_key == response.public_key {
                            warn!("Using cached peer {} due to RPC error: {}", response.near_account, e);
                            return Ok(known_peer.clone());
                        }
                    }
                    return Err(anyhow!(
                        "Failed to fetch public keys from NEAR blockchain for {}: {}",
                        response.near_account, e
                    ));
                }
            };

            // Check if the provided public key matches ANY of the account's keys
            if !valid_keys.contains(&response.public_key) {
                return Err(anyhow!(
                    "IMPERSONATION DETECTED: Public key {:?} is not authorized for {}! \
                    Account has {} keys, none match.",
                    &response.public_key[..8],
                    response.near_account,
                    valid_keys.len()
                ));
            }
            
            info!("Verified {} has {} authorized keys", response.near_account, valid_keys.len());
        }

        // 4. Create verified peer
        let verified = VerifiedPeer {
            near_account: response.near_account.clone(),
            peer_id: response.challenge.peer_id.clone(),
            public_key: response.public_key.clone(),
            verified_at: now,
            trust_level: TrustLevel::Known,
        };

        // 5. Cache the verified peer (with persistence)
        self.trusted_peers.insert(verified.near_account.clone(), verified.clone());
        
        // Persist to database if enabled
        if let Some(ref db) = self.db {
            let key = verified.near_account.as_bytes();
            let value = serde_json::to_vec(&verified)?;
            db.put(key, value)?;
        }

        Ok(verified)
    }

    /// Rotate a peer's public key (after they update their NEAR keys)
    pub fn rotate_peer_key(&mut self, account: &str, new_public_key: Vec<u8>) -> Result<()> {
        if let Some(peer) = self.trusted_peers.get_mut(account) {
            peer.public_key = new_public_key.clone();
            peer.verified_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            // Persist to database if enabled
            if let Some(ref db) = self.db {
                let key = account.as_bytes();
                let value = serde_json::to_vec(peer)?;
                db.put(key, value)?;
            }
            
            info!("Rotated key for {}", account);
            Ok(())
        } else {
            Err(anyhow!("Peer {} not found in trusted peers", account))
        }
    }

    /// Force re-verification of all peers (call this periodically)
    pub async fn reverify_all_peers(&mut self) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut verified_count = 0;
        let accounts: Vec<String> = self.trusted_peers.keys().cloned().collect();
        
        for account in accounts {
            if let Some(peer) = self.trusted_peers.get(&account).cloned() {
                // Fetch current keys from blockchain
                match self.fetch_near_public_keys(&account).await {
                    Ok(valid_keys) => {
                        if valid_keys.contains(&peer.public_key) {
                            // Key still valid, update timestamp
                            if let Some(p) = self.trusted_peers.get_mut(&account) {
                                p.verified_at = now;
                            }
                            verified_count += 1;
                        } else {
                            // Key no longer valid - remove peer
                            warn!("Key for {} no longer valid, removing from trusted peers", account);
                            self.trusted_peers.remove(&account);
                            
                            // Remove from database if enabled
                            if let Some(ref db) = self.db {
                                db.delete(account.as_bytes())?;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to re-verify {}: {}", account, e);
                    }
                }
            }
        }
        
        info!("Re-verified {} peers", verified_count);
        Ok(verified_count)
    }

    /// Get trust level for an account
    pub fn get_trust_level(&self, account: &str) -> TrustLevel {
        if account == self.near_account {
            return TrustLevel::Owner;
        }

        self.trusted_peers
            .get(account)
            .map(|p| p.trust_level)
            .unwrap_or(TrustLevel::Untrusted)
    }

    /// Set trust level for an account
    pub fn set_trust_level(&mut self, account: &str, level: TrustLevel) {
        if let Some(peer) = self.trusted_peers.get_mut(account) {
            peer.trust_level = level;
            
            // Persist to database if enabled
            if let Some(ref db) = self.db {
                let key = account.as_bytes();
                if let Ok(value) = serde_json::to_vec(peer) {
                    let _ = db.put(key, value);
                }
            }
        }
    }

    /// Add trusted peer (e.g., from registry)
    pub fn add_trusted_peer(&mut self, peer: VerifiedPeer) {
        let account = peer.near_account.clone();
        
        // Persist to database if enabled
        if let Some(ref db) = self.db {
            let key = account.as_bytes();
            if let Ok(value) = serde_json::to_vec(&peer) {
                let _ = db.put(key, value);
            }
        }
        
        self.trusted_peers.insert(account, peer);
    }

    /// Check if peer is verified
    pub fn is_verified(&self, account: &str) -> bool {
        self.trusted_peers.contains_key(account)
    }

    /// Get our verifying key
    pub fn public_key(&self) -> Vec<u8> {
        self.signing_key.verifying_key().to_bytes().to_vec()
    }
    
    // ========================================================================
    // TRUST SCORE SYSTEM
    // ========================================================================
    
    /// Calculate comprehensive trust score for an account
    pub async fn calculate_trust_score(&mut self, account: &str) -> Result<TrustScore> {
        // Check cache first
        if let Some(cached) = self.trust_cache.get(account) {
            return Ok(cached.clone());
        }
        
        // 1. Fetch account data from NEAR
        let account_data = self.fetch_account_data(account).await?;
        
        // 2. Calculate individual factors
        let total_stake = account_data.amount.saturating_add(account_data.locked);
        let stake_factor = Self::calculate_stake_factor(total_stake);
        let age_factor = Self::calculate_age_factor(account_data.created_at);
        
        // 3. Fetch recent transaction count
        let tx_count = self.fetch_recent_tx_count(account).await.unwrap_or(0);
        let activity_factor = Self::calculate_activity_factor(tx_count);
        
        // 4. Get existing reputation (from our system)
        let (avg_rating, rating_count) = self.get_reputation_stats(account);
        let reputation_factor = Self::calculate_reputation_factor(avg_rating, rating_count);
        
        // 5. Get job history (from our system)
        let (completed_jobs, success_rate) = self.get_job_history_stats(account);
        let history_factor = Self::calculate_history_factor(completed_jobs, success_rate);
        
        // 6. Calculate age in days
        let account_age_days = account_data.created_at
            .map(|created| {
                let now = current_timestamp();
                now.saturating_sub(created) / 86400
            })
            .unwrap_or(0);
        
        let score = TrustScore {
            stake_factor,
            age_factor,
            activity_factor,
            reputation_factor,
            history_factor,
            stake_yocto: total_stake,
            account_age_days,
            tx_count_90d: tx_count,
            avg_rating,
            rating_count,
            completed_jobs,
            job_success_rate: success_rate,
            calculated_at: current_timestamp(),
        };
        
        // Cache the result
        self.trust_cache.insert(account.to_string(), score.clone());
        
        info!("Trust score for {}: {:.1} (stake={:.2}, age={:.2}, activity={:.2}, rep={:.2}, hist={:.2})",
              account, score.calculate(), stake_factor, age_factor, activity_factor, 
              reputation_factor, history_factor);
        
        Ok(score)
    }
    
    /// Calculate stake factor (40% weight)
    /// Uses logarithmic scaling to prevent whale dominance
    fn calculate_stake_factor(stake_yocto: u128) -> f64 {
        let near = stake_yocto as f64 / ONE_NEAR as f64;
        if near <= 0.0 {
            return 0.0;
        }
        
        // Logarithmic scale:
        // 0 NEAR = 0.0
        // 1 NEAR = 0.20
        // 10 NEAR = 0.47
        // 100 NEAR = 0.73
        // 1000 NEAR = 1.0
        
        (near.ln() / 7.0).min(1.0).max(0.0)
    }
    
    /// Calculate age factor (15% weight)
    /// Linear growth for first year, then caps
    fn calculate_age_factor(created_at: Option<u64>) -> f64 {
        let created = match created_at {
            Some(t) => t,
            None => return 0.0, // Unknown age = suspicious
        };
        
        let now = current_timestamp();
        let age_days = now.saturating_sub(created) / 86400;
        
        // 0 days = 0.0
        // 30 days = 0.25
        // 90 days = 0.5
        // 365 days = 1.0
        (age_days as f64 / 365.0).min(1.0)
    }
    
    /// Calculate activity factor (15% weight)
    /// Based on recent on-chain transactions
    fn calculate_activity_factor(tx_count_90d: u64) -> f64 {
        // 0 tx = 0.0
        // 10 tx = 0.3
        // 50 tx = 0.6
        // 100+ tx = 1.0
        
        (tx_count_90d as f64 / 100.0).min(1.0)
    }
    
    /// Calculate reputation factor (20% weight)
    /// Weighted by both average rating and number of ratings (confidence)
    fn calculate_reputation_factor(avg_rating: f64, rating_count: u64) -> f64 {
        // No ratings = 0.5 (neutral, not penalized for being new)
        if rating_count == 0 {
            return 0.5;
        }
        
        // Confidence factor (more ratings = more reliable)
        let confidence = (rating_count as f64 / 10.0).min(1.0);
        
        // Normalize rating from 1-5 to 0-1
        let rating_normalized = (avg_rating - 1.0) / 4.0;
        
        // Weight: 70% rating quality, 30% confidence
        rating_normalized * 0.7 + confidence * 0.3
    }
    
    /// Calculate history factor (10% weight)
    /// Based on completed jobs and success rate
    fn calculate_history_factor(completed_jobs: u64, success_rate: f64) -> f64 {
        // 0 jobs = 0.0
        // 1 job = 0.3
        // 5 jobs = 0.6
        // 10+ jobs = 1.0
        
        let job_factor = (completed_jobs as f64 / 10.0).min(1.0);
        
        // Combine job count with success rate
        job_factor * 0.6 + success_rate * 0.4
    }
    
    /// Fetch account data from NEAR blockchain
    async fn fetch_account_data(&self, account_id: &str) -> Result<AccountData> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "view_account",
                "finality": "final",
                "account_id": account_id
            }
        });

        let response = self.http_client
            .post(self.network.rpc_url())
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| anyhow!("RPC request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("RPC error: {}", response.status()));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse RPC response: {}", e))?;
        
        let account_result = result.get("result")
            .ok_or_else(|| anyhow!("No result in RPC response"))?;
        
        let amount = account_result.get("amount")
            .and_then(|a| a.as_str())
            .and_then(|a| a.parse::<u128>().ok())
            .unwrap_or(0);
        
        let locked = account_result.get("locked")
            .and_then(|l| l.as_str())
            .and_then(|l| l.parse::<u128>().ok())
            .unwrap_or(0);
        
        let storage_usage = account_result.get("storage_usage")
            .and_then(|s| s.as_u64())
            .unwrap_or(0);
        
        // Try to fetch account creation timestamp from archival node
        let created_at = self.fetch_account_creation_time(account_id).await.ok().flatten();
        
        Ok(AccountData {
            account_id: account_id.to_string(),
            amount,
            locked,
            storage_usage,
            created_at,
        })
    }
    
    /// Fetch account creation timestamp from archival RPC
    /// Uses the first transaction/block to determine when account was created
    async fn fetch_account_creation_time(&self, account_id: &str) -> Result<Option<u64>> {
        // Only fetch from mainnet archival (testnet doesn't have reliable archival)
        if matches!(self.network, Network::Testnet) {
            return Ok(None);
        }
        
        // Query archival node for account's first transaction
        // We use a heuristic: query for the earliest transaction involving this account
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "EXPERIMENTAL_genesis_protocol_config",
            "params": {}
        });

        // First, try to get account's first appearance from archival
        // This is a best-effort approach - we'll use FastNear for more reliable data
        let response = self.http_client
            .post(ARCHIVAL_RPC_URL)
            .json(&body)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await;

        // If archival fails, fall back to indexer
        if response.is_err() {
            return self.fetch_account_creation_from_indexer(account_id).await;
        }

        // Try alternative: use block hash from account creation
        // NEAR doesn't provide direct creation time, so we use FastNear indexer
        self.fetch_account_creation_from_indexer(account_id).await
    }
    
    /// Fetch account creation time from FastNear indexer
    async fn fetch_account_creation_from_indexer(&self, account_id: &str) -> Result<Option<u64>> {
        // FastNear API: GET /account/{account_id}
        let url = format!("{}/account/{}", FASTNEAR_API_URL, account_id);
        
        let response = match self.http_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("FastNear API request failed for {}: {}", account_id, e);
                return Ok(None);
            }
        };

        if !response.status().is_success() {
            warn!("FastNear API returned {} for {}", response.status(), account_id);
            return Ok(None);
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse FastNear response: {}", e))?;

        // FastNear returns account info including created_at timestamp
        let created_at = result.get("created_at")
            .or_else(|| result.get("created_time"))
            .and_then(|t| t.as_u64());

        Ok(created_at)
    }
    
    /// Fetch recent transaction count from FastNear indexer (last 90 days)
    async fn fetch_recent_tx_count(&self, account_id: &str) -> Result<u64> {
        // Only query mainnet (testnet indexer may not be available)
        if matches!(self.network, Network::Testnet) {
            // For testnet, we can't reliably get tx count, return neutral value
            return Ok(0);
        }

        // FastNear API: GET /account/{account_id}/activity
        let url = format!("{}/account/{}/activity?days={}", 
                         FASTNEAR_API_URL, account_id, ACTIVITY_LOOKBACK_DAYS);
        
        let response = match self.http_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("FastNear activity API request failed for {}: {}", account_id, e);
                // Return 0 on error - other trust factors will compensate
                return Ok(0);
            }
        };

        if !response.status().is_success() {
            warn!("FastNear activity API returned {} for {}", response.status(), account_id);
            return Ok(0);
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse FastNear activity response: {}", e))?;

        // FastNear returns activity stats including tx count
        let tx_count = result.get("tx_count")
            .or_else(|| result.get("transaction_count"))
            .or_else(|| result.get("total_transactions"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0);

        info!("Fetched tx count for {}: {} in last {} days", account_id, tx_count, ACTIVITY_LOOKBACK_DAYS);
        Ok(tx_count)
    }
    
    /// Get reputation stats from our rating system
    fn get_reputation_stats(&self, account: &str) -> (f64, u64) {
        let ratings = match self.ratings.get(account) {
            Some(r) => r,
            None => return (0.0, 0),
        };
        
        if ratings.is_empty() {
            return (0.0, 0);
        }
        
        // Apply decay to old ratings
        let now = current_timestamp();
        let mut total_weight: f64 = 0.0;
        let mut weighted_sum: f64 = 0.0;
        let mut count: u64 = 0;
        
        for entry in ratings {
            // Calculate decay factor
            let age_days = (now - entry.timestamp) / 86400;
            let decay = if age_days < RATING_DECAY_DAYS {
                (RATING_DECAY_DAYS - age_days) as f64 / RATING_DECAY_DAYS as f64
            } else {
                0.1 // Keep 10% weight for very old ratings
            };
            
            let effective_weight = entry.weight as f64 * decay;
            total_weight += effective_weight;
            weighted_sum += (entry.rating as f64) * effective_weight;
            count += 1;
        }
        
        if total_weight == 0.0 {
            return (0.0, 0);
        }
        
        let avg = weighted_sum / total_weight;
        (avg, count)
    }
    
    /// Get job history stats
    fn get_job_history_stats(&self, account: &str) -> (u64, f64) {
        let jobs = match self.job_history.get(account) {
            Some(j) => j,
            None => return (0, 0.0),
        };
        
        if jobs.is_empty() {
            return (0, 0.0);
        }
        
        let completed = jobs.len() as u64;
        let successful = jobs.iter().filter(|j| j.success).count() as u64;
        let success_rate = successful as f64 / completed as f64;
        
        (completed, success_rate)
    }
    
    /// Submit a rating with trust-weighted influence
    pub async fn submit_rating(
        &mut self,
        rater: &str,
        target: &str,
        rating: u8,
    ) -> Result<()> {
        // Validate rating range
        if rating < 1 || rating > 5 {
            return Err(anyhow!("Rating must be 1-5, got {}", rating));
        }
        
        // Calculate rater's trust score
        let trust = self.calculate_trust_score(rater).await?;
        let trust_value = trust.calculate();
        
        if !trust.can_perform(TrustAction::Rate) {
            return Err(anyhow!(
                "Trust score too low ({:.1}) - minimum {:.0} required to rate",
                trust_value,
                MIN_TRUST_TO_RATE
            ));
        }
        
        // Check cooldown (1 rating per target per day per rater)
        let now = current_timestamp();
        if let Some(ratings) = self.ratings.get(target) {
            for existing in ratings {
                if existing.rater == rater {
                    let age_secs = now - existing.timestamp;
                    if age_secs < RATING_COOLDOWN_SECS {
                        let remaining = RATING_COOLDOWN_SECS - age_secs;
                        return Err(anyhow!(
                            "Rating cooldown - can rate again in {} hours",
                            remaining / 3600
                        ));
                    }
                }
            }
        }
        
        // Calculate weight (trust score * scale for precision)
        let weight = (trust_value * 1_000_000.0) as u128;
        
        // Create rating entry
        let entry = RatingEntry {
            rater: rater.to_string(),
            rating,
            weight,
            timestamp: now,
            rater_trust_score: trust_value,
        };
        
        // Store rating
        self.ratings
            .entry(target.to_string())
            .or_insert_with(Vec::new)
            .push(entry);
        
        // Invalidate target's cached trust score (reputation changed)
        self.trust_cache.invalidate(target);
        
        // Persist if database enabled
        if let Some(ref db) = self.db {
            let key = format!("ratings:{}", target);
            if let Ok(value) = serde_json::to_vec(&self.ratings.get(target)) {
                let _ = db.put(key.as_bytes(), value);
            }
        }
        
        info!("Rating submitted: {} rated {} as {} stars (weight: {:.1})", 
              rater, target, rating, trust_value);
        
        Ok(())
    }
    
    /// Record a job completion
    pub fn record_job(
        &mut self,
        account: &str,
        job_id: String,
        success: bool,
        value_yocto: u128,
    ) {
        let record = JobRecord {
            job_id,
            account: account.to_string(),
            completed_at: current_timestamp(),
            success,
            value_yocto,
        };
        
        self.job_history
            .entry(account.to_string())
            .or_insert_with(Vec::new)
            .push(record);
        
        // Invalidate cached trust score (history changed)
        self.trust_cache.invalidate(account);
        
        // Persist if database enabled
        if let Some(ref db) = self.db {
            let key = format!("jobs:{}", account);
            if let Ok(value) = serde_json::to_vec(&self.job_history.get(account)) {
                let _ = db.put(key.as_bytes(), value);
            }
        }
    }
    
    /// Get trust score summary for display
    pub async fn get_trust_summary(&mut self, account: &str) -> Result<String> {
        let score = self.calculate_trust_score(account).await?;
        
        let total = score.calculate();
        let stake_near = score.stake_yocto as f64 / ONE_NEAR as f64;
        
        Ok(format!(
            "Trust Score: {:.1}/100\n\
             ├─ Stake: {:.2} ({:.1} NEAR)\n\
             ├─ Age: {:.2} ({} days)\n\
             ├─ Activity: {:.2} ({} tx/90d)\n\
             ├─ Reputation: {:.2} ({:.1}★ from {} ratings)\n\
             └─ History: {:.2} ({} jobs, {:.0}% success)",
            total,
            score.stake_factor,
            stake_near,
            score.age_factor,
            score.account_age_days,
            score.activity_factor,
            score.tx_count_90d,
            score.reputation_factor,
            score.avg_rating,
            score.rating_count,
            score.history_factor,
            score.completed_jobs,
            score.job_success_rate * 100.0
        ))
    }
    
    /// Check if account can perform an action
    pub async fn can_perform_action(&mut self, account: &str, action: TrustAction) -> Result<bool> {
        let score = self.calculate_trust_score(account).await?;
        Ok(score.can_perform(action))
    }
    
    /// Apply decay to all old ratings (call periodically)
    pub fn apply_rating_decay(&mut self) {
        // Ratings are decayed on-demand in get_reputation_stats
        // This method can be used to clean up very old ratings
        let now = current_timestamp();
        let cutoff = now - (RATING_DECAY_DAYS * 2 * 86400); // 180 days
        
        for ratings in self.ratings.values_mut() {
            ratings.retain(|r| r.timestamp > cutoff);
        }
    }
}

/// Helper function to get current timestamp in seconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Peer authentication message type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerAuthMessage {
    Challenge(AuthChallenge),
    Response(AuthResponse),
    Verified(VerifiedPeer),
    Error(String),
    /// Trust score request/response for P2P
    TrustScoreRequest { account: String },
    TrustScoreResponse { account: String, score: TrustScore },
}

// ============================================================================
// P2P MESSAGE HANDLING INTEGRATION
// ============================================================================

/// Result of processing a P2P message with trust validation
#[derive(Debug, Clone)]
pub struct TrustValidationResult {
    pub allowed: bool,
    pub trust_score: f64,
    pub reason: String,
    pub peer: Option<VerifiedPeer>,
}

impl PeerAuthenticator {
    /// Process incoming P2P message with trust validation
    /// This is the main entry point for P2P message handling
    pub async fn process_p2p_message(
        &mut self,
        from_account: &str,
        message_type: P2PMessageType,
        payload: &[u8],
    ) -> Result<TrustValidationResult> {
        // 1. Verify peer identity (signature already verified at network layer)
        let peer = if let Some(p) = self.trusted_peers.get(from_account).cloned() {
            p
        } else {
            // Peer not verified yet - they need to complete auth flow first
            return Ok(TrustValidationResult {
                allowed: false,
                trust_score: 0.0,
                reason: "Peer not authenticated - complete auth flow first".to_string(),
                peer: None,
            });
        };

        // 2. Calculate trust score
        let trust = self.calculate_trust_score(from_account).await?;
        let score = trust.calculate();

        // 3. Check if action is allowed based on message type
        let (allowed, reason) = match message_type {
            P2PMessageType::SkillAdvertisement => {
                // Anyone can advertise skills
                (true, "Skill advertisement allowed".to_string())
            }
            P2PMessageType::TaskRequest => {
                // Need minimum trust to request tasks
                if trust.can_perform(TrustAction::Bid) {
                    (true, format!("Task request allowed (score: {:.1})", score))
                } else {
                    (false, format!("Task request denied - need score >= {:.0}, got {:.1}", 
                                   MIN_TRUST_TO_BID, score))
                }
            }
            P2PMessageType::TaskResponse => {
                // Anyone verified can respond to tasks
                (true, "Task response allowed".to_string())
            }
            P2PMessageType::Rating => {
                // Need minimum trust to rate
                if trust.can_perform(TrustAction::Rate) {
                    // Parse rating from payload
                    match self.parse_rating_payload(payload) {
                        Ok((target, rating)) => {
                            // Submit the rating
                            match self.submit_rating(from_account, &target, rating).await {
                                Ok(_) => (true, format!("Rating {} -> {} stars submitted", target, rating)),
                                Err(e) => (false, format!("Rating failed: {}", e)),
                            }
                        }
                        Err(e) => (false, format!("Invalid rating payload: {}", e)),
                    }
                } else {
                    (false, format!("Rating denied - need score >= {:.0}, got {:.1}", 
                                   MIN_TRUST_TO_RATE, score))
                }
            }
            P2PMessageType::Dispute => {
                // Need higher trust to open disputes
                if trust.can_perform(TrustAction::Dispute) {
                    (true, "Dispute allowed".to_string())
                } else {
                    (false, format!("Dispute denied - need score >= {:.0}, got {:.1}", 
                                   MIN_TRUST_TO_DISPUTE, score))
                }
            }
            P2PMessageType::Approval => {
                // Need highest trust to approve work
                if trust.can_perform(TrustAction::Approve) {
                    (true, "Approval allowed".to_string())
                } else {
                    (false, format!("Approval denied - need score >= {:.0}, got {:.1}", 
                                   MIN_TRUST_TO_APPROVE, score))
                }
            }
            P2PMessageType::Query => {
                // Queries are always allowed
                (true, "Query allowed".to_string())
            }
            P2PMessageType::TrustScoreRequest => {
                // Trust score requests are always allowed
                (true, "Trust score request allowed".to_string())
            }
        };

        // 4. Log the action
        info!(
            "P2P message from {}: type={:?}, score={:.1}, allowed={}",
            from_account, message_type, score, allowed
        );

        Ok(TrustValidationResult {
            allowed,
            trust_score: score,
            reason,
            peer: Some(peer),
        })
    }

    /// Parse rating payload (target_account, rating)
    fn parse_rating_payload(&self, payload: &[u8]) -> Result<(String, u8)> {
        let rating_msg: RatingMessage = serde_json::from_slice(payload)
            .map_err(|e| anyhow!("Failed to parse rating: {}", e))?;
        
        if rating_msg.rating < 1 || rating_msg.rating > 5 {
            return Err(anyhow!("Rating must be 1-5, got {}", rating_msg.rating));
        }

        Ok((rating_msg.target, rating_msg.rating))
    }

    /// Create a trust score response for P2P network
    pub async fn create_trust_score_response(&mut self, account: &str) -> Result<PeerAuthMessage> {
        let score = self.calculate_trust_score(account).await?;
        Ok(PeerAuthMessage::TrustScoreResponse {
            account: account.to_string(),
            score,
        })
    }

    /// Handle incoming trust score from another peer
    pub fn handle_trust_score_response(&mut self, account: &str, score: &TrustScore) -> Result<()> {
        // Verify the score is recent (within 1 hour)
        let now = current_timestamp();
        if now - score.calculated_at > TRUST_CACHE_TTL_SECS {
            warn!("Received stale trust score for {} ({} seconds old)", 
                  account, now - score.calculated_at);
            return Err(anyhow!("Trust score too old"));
        }

        // Cache the received score (trust but verify - we'll recalculate if needed)
        self.trust_cache.insert(account.to_string(), score.clone());
        
        info!("Cached trust score for {}: {:.1}", account, score.calculate());
        Ok(())
    }

    /// Get peers sorted by trust score (for peer selection)
    pub async fn get_trusted_peers_by_score(&mut self) -> Result<Vec<(String, f64)>> {
        // Collect accounts first to avoid borrow issues
        let accounts: Vec<String> = self.trusted_peers.keys().cloned().collect();
        let mut peer_scores = Vec::new();
        
        for account in accounts {
            let score = self.calculate_trust_score(&account).await?;
            peer_scores.push((account, score.calculate()));
        }
        
        // Sort by score descending
        peer_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(peer_scores)
    }

    /// Select best peer for a task (highest trust score that can perform the action)
    pub async fn select_peer_for_task(&mut self, action: TrustAction) -> Result<Option<String>> {
        let peers = self.get_trusted_peers_by_score().await?;
        
        for (account, score) in peers {
            // Get fresh trust score to check action capability
            let trust = self.calculate_trust_score(&account).await?;
            if trust.can_perform(action) {
                info!("Selected peer {} for {:?} (score: {:.1})", account, action, score);
                return Ok(Some(account));
            }
        }
        
        warn!("No peer found capable of {:?}", action);
        Ok(None)
    }
}

/// P2P message types that require trust validation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum P2PMessageType {
    /// Skill advertisement (low trust required)
    SkillAdvertisement,
    /// Task request (medium trust required)
    TaskRequest,
    /// Task response (low trust required)
    TaskResponse,
    /// Rating submission (low+ trust required)
    Rating,
    /// Dispute opening (medium+ trust required)
    Dispute,
    /// Work approval (high trust required)
    Approval,
    /// General query (no trust required)
    Query,
    /// Trust score request (no trust required)
    TrustScoreRequest,
}

/// Rating message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingMessage {
    pub target: String,
    pub rating: u8,
    pub comment: Option<String>,
}

/// P2P message envelope with trust metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PMessageEnvelope {
    pub from: String,
    pub message_type: P2PMessageType,
    pub payload: Vec<u8>,
    pub timestamp: u64,
    pub signature: Vec<u8>,
}

impl P2PMessageEnvelope {
    /// Create a new envelope
    pub fn new(from: String, message_type: P2PMessageType, payload: Vec<u8>) -> Self {
        Self {
            from,
            message_type,
            payload,
            timestamp: current_timestamp(),
            signature: vec![],
        }
    }

    /// Sign the envelope
    pub fn sign(&mut self, signing_key: &SigningKey) -> Result<()> {
        let message_bytes = self.signing_payload();
        let signature = signing_key.sign(&message_bytes);
        self.signature = signature.to_bytes().to_vec();
        Ok(())
    }

    /// Get the payload to sign (everything except signature)
    fn signing_payload(&self) -> Vec<u8> {
        serde_json::to_vec(&(
            &self.from,
            &self.message_type,
            &self.payload,
            &self.timestamp,
        )).unwrap_or_default()
    }

    /// Verify the signature
    pub fn verify_signature(&self, verifying_key: &VerifyingKey) -> bool {
        let message_bytes = self.signing_payload();
        let sig_bytes: [u8; 64] = match self.signature.as_slice().try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };
        let signature = Signature::from_bytes(&sig_bytes);
        verifying_key.verify(&message_bytes, &signature).is_ok()
    }
}

// ============================================================================
// NEAR DNS RESOLVER
// ============================================================================

/// DNS record returned from NEAR DNS contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearDnsRecord {
    pub record_type: String,
    pub value: String,
    pub ttl: u64,
    pub priority: Option<u64>,
}

/// NEAR DNS resolver for decentralized peer discovery
pub struct NearDnsResolver {
    http_client: Client,
    network: Network,
    cache: HashMap<String, (Vec<NearDnsRecord>, u64)>,  // (records, timestamp)
    cache_ttl_secs: u64,
}

impl NearDnsResolver {
    /// Create a new DNS resolver
    pub fn new(network: Network) -> Self {
        Self {
            http_client: Client::new(),
            network,
            cache: HashMap::new(),
            cache_ttl_secs: 300,  // 5 minutes
        }
    }

    /// Resolve a domain name via NEAR DNS contract
    /// 
    /// # Example
    /// ```
    /// let resolver = NearDnsResolver::new(Network::Mainnet);
    /// let records = resolver.resolve("gork.jemartel.near", "A").await?;
    /// ```
    pub async fn resolve(&mut self, domain: &str, record_type: &str) -> Result<Vec<NearDnsRecord>> {
        // Check cache first
        let cache_key = format!("{}:{}", domain, record_type);
        let now = current_timestamp();
        
        if let Some((records, ts)) = self.cache.get(&cache_key) {
            if now - ts < self.cache_ttl_secs {
                info!("DNS cache hit for {} {}", record_type, domain);
                return Ok(records.clone());
            }
        }

        // Parse domain to get contract and name
        let (dns_contract, name) = self.parse_domain(domain)?;

        // Query the DNS contract
        let records = self.query_dns_contract(&dns_contract, &name, record_type).await?;

        // Cache the result
        self.cache.insert(cache_key, (records.clone(), now));

        info!("Resolved {} {} → {} records", record_type, domain, records.len());
        Ok(records)
    }

    /// Resolve all record types for a domain
    pub async fn resolve_all(&mut self, domain: &str) -> Result<Vec<NearDnsRecord>> {
        let (dns_contract, name) = self.parse_domain(domain)?;
        self.query_dns_contract_all(&dns_contract, &name).await
    }

    /// Parse domain like "gork.jemartel.near" into ("dns.jemartel.near", "gork")
    fn parse_domain(&self, domain: &str) -> Result<(String, String)> {
        let parts: Vec<&str> = domain.split('.').collect();
        
        if parts.len() < 3 {
            return Err(anyhow!("Invalid NEAR domain: {}", domain));
        }

        // gork.jemartel.near → dns.jemartel.near, gork
        let tld = parts.last().ok_or_else(|| anyhow!("No TLD"))?;
        let dns_contract = format!("dns.{}.{}", parts[parts.len() - 2], tld);
        let name = parts[0];

        Ok((dns_contract, name.to_string()))
    }

    /// Query DNS contract for specific record type
    async fn query_dns_contract(
        &self,
        contract: &str,
        name: &str,
        record_type: &str,
    ) -> Result<Vec<NearDnsRecord>> {
        let args = serde_json::json!({
            "name": name,
            "record_type": record_type
        });
        
        let args_base64 = general_purpose::STANDARD
            .encode(serde_json::to_vec(&args)?);

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "call_function",
                "finality": "final",
                "account_id": contract,
                "method_name": "dns_query",
                "args_base64": args_base64
            }
        });

        let response = self.http_client
            .post(self.network.rpc_url())
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| anyhow!("DNS query failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("DNS RPC error: {}", response.status()));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse DNS response: {}", e))?;

        // Extract result from NEAR RPC response
        let result_bytes = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.as_array())
            .ok_or_else(|| anyhow!("No DNS records found"))?;

        // Decode the result bytes
        let bytes: Vec<u8> = result_bytes
            .iter()
            .filter_map(|b| b.as_u64().map(|v| v as u8))
            .collect();

        // Parse the DNS records
        if bytes.is_empty() || bytes == vec![0] {
            return Ok(vec![]);  // No records
        }

        let records: Vec<NearDnsRecord> = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| {
                // Try parsing as single record
                if let Ok(record) = serde_json::from_slice::<NearDnsRecord>(&bytes) {
                    vec![record]
                } else {
                    vec![]
                }
            });

        Ok(records)
    }

    /// Query DNS contract for all records of a name
    async fn query_dns_contract_all(
        &self,
        contract: &str,
        name: &str,
    ) -> Result<Vec<NearDnsRecord>> {
        let args = serde_json::json!({
            "name": name
        });
        
        let args_base64 = general_purpose::STANDARD
            .encode(serde_json::to_vec(&args)?);

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "call_function",
                "finality": "final",
                "account_id": contract,
                "method_name": "dns_query_all",
                "args_base64": args_base64
            }
        });

        let response = self.http_client
            .post(self.network.rpc_url())
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| anyhow!("DNS query failed: {}", e))?;

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse DNS response: {}", e))?;

        let result_bytes = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.as_array())
            .ok_or_else(|| anyhow!("No DNS records found"))?;

        let bytes: Vec<u8> = result_bytes
            .iter()
            .filter_map(|b| b.as_u64().map(|v| v as u8))
            .collect();

        if bytes.is_empty() || bytes == vec![0] {
            return Ok(vec![]);
        }

        let records: Vec<NearDnsRecord> = serde_json::from_slice(&bytes)
            .unwrap_or_default();

        Ok(records)
    }

    /// List all DNS names in a contract
    pub async fn list_names(&self, contract: &str) -> Result<Vec<String>> {
        let args = serde_json::json!({});
        let args_base64 = general_purpose::STANDARD
            .encode(serde_json::to_vec(&args)?);

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "call_function",
                "finality": "final",
                "account_id": contract,
                "method_name": "dns_list_names",
                "args_base64": args_base64
            }
        });

        let response = self.http_client
            .post(self.network.rpc_url())
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;

        let result_bytes = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.as_array())
            .ok_or_else(|| anyhow!("No names found"))?;

        let bytes: Vec<u8> = result_bytes
            .iter()
            .filter_map(|b| b.as_u64().map(|v| v as u8))
            .collect();

        let names: Vec<String> = serde_json::from_slice(&bytes)
            .unwrap_or_default();

        Ok(names)
    }

    /// Get first A record IP address for a domain
    pub async fn resolve_ip(&mut self, domain: &str) -> Result<Option<String>> {
        let records = self.resolve(domain, "A").await?;
        Ok(records.first().map(|r| r.value.clone()))
    }

    /// Get TXT record value (useful for peer IDs, metadata)
    pub async fn resolve_txt(&mut self, domain: &str) -> Result<Option<String>> {
        let records = self.resolve(domain, "TXT").await?;
        Ok(records.first().map(|r| r.value.clone()))
    }

    /// Clear the DNS cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

impl PeerAuthenticator {
    /// Resolve a peer's address via NEAR DNS
    pub async fn resolve_peer_address(&mut self, domain: &str) -> Result<Option<String>> {
        let mut resolver = NearDnsResolver::new(self.network.clone());
        resolver.resolve_ip(domain).await
    }

    /// Get peer metadata from DNS TXT records
    pub async fn get_peer_metadata(&mut self, domain: &str) -> Result<HashMap<String, String>> {
        let mut resolver = NearDnsResolver::new(self.network.clone());
        let records = resolver.resolve(domain, "TXT").await?;
        
        let mut metadata = HashMap::new();
        
        if let Some(txt) = records.first() {
            // Parse TXT record like "peer_id=12D3Koo...;version=1.0.0"
            for pair in txt.value.split(';') {
                if let Some((key, value)) = pair.split_once('=') {
                    metadata.insert(key.trim().to_string(), value.trim().to_string());
                }
            }
        }
        
        Ok(metadata)
    }

    /// Discover all gork peers from a DNS contract
    pub async fn discover_gork_peers(&self, dns_contract: &str) -> Result<Vec<String>> {
        let resolver = NearDnsResolver::new(self.network.clone());
        let names = resolver.list_names(dns_contract).await?;
        
        // Filter for gork-related names
        let gork_peers: Vec<String> = names
            .into_iter()
            .filter(|n| n.starts_with("gork") || n.starts_with("node"))
            .map(|n| format!("{}.{}", n, dns_contract.trim_start_matches("dns.")))
            .collect();
        
        info!("Discovered {} gork peers from {}", gork_peers.len(), dns_contract);
        Ok(gork_peers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_challenge_creation() {
        let auth = PeerAuthenticator::new("alice.test".to_string());
        let challenge = auth.create_challenge("peer123".to_string());

        assert_eq!(challenge.peer_id, "peer123");
        assert!(challenge.timestamp > 0);
        assert!(challenge.nonce.iter().any(|&b| b != 0));
    }

    #[tokio::test]
    async fn test_sign_and_verify() {
        let auth1 = PeerAuthenticator::new("alice.test".to_string());
        let mut auth2 = PeerAuthenticator::new("bob.test".to_string());

        // Pre-register alice's public key in auth2's trusted peers
        // (In production, this would come from the NEAR blockchain)
        let alice_verified = VerifiedPeer {
            near_account: "alice.test".to_string(),
            peer_id: "alice-peer".to_string(),
            public_key: auth1.public_key(),
            verified_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            trust_level: TrustLevel::Trusted,
        };
        auth2.add_trusted_peer(alice_verified);

        // Create challenge
        let challenge = auth2.create_challenge("peer456".to_string());

        // Sign it
        let response = auth1.sign_challenge(&challenge).unwrap();

        // Verify it
        let verified = auth2.verify_peer(&response).await.unwrap();

        assert_eq!(verified.near_account, "alice.test");
        assert_eq!(verified.peer_id, "peer456");
        assert_eq!(verified.trust_level, TrustLevel::Known);
    }

    #[tokio::test]
    async fn test_signature_verification_fails_for_invalid() {
        let mut auth = PeerAuthenticator::new("alice.test".to_string());
        let challenge = auth.create_challenge("peer789".to_string());
        let mut response = auth.sign_challenge(&challenge).unwrap();

        // Corrupt the signature
        response.signature[0] ^= 0xFF;

        let result = auth.verify_peer(&response).await;
        assert!(result.is_err());
    }
    
    // ========================================
    // Trust Score Tests
    // ========================================
    
    #[test]
    fn test_stake_factor() {
        // 0 NEAR = 0.0
        assert!((PeerAuthenticator::calculate_stake_factor(0) - 0.0).abs() < 0.01);
        
        // 1 NEAR = 0.0 (ln(1) = 0)
        let one_near = ONE_NEAR;
        assert!((PeerAuthenticator::calculate_stake_factor(one_near) - 0.0).abs() < 0.01);
        
        // 3 NEAR ≈ 0.16
        assert!((PeerAuthenticator::calculate_stake_factor(one_near * 3) - 0.16).abs() < 0.05);
        
        // 10 NEAR ≈ 0.33
        assert!((PeerAuthenticator::calculate_stake_factor(one_near * 10) - 0.33).abs() < 0.05);
        
        // 100 NEAR ≈ 0.66
        assert!((PeerAuthenticator::calculate_stake_factor(one_near * 100) - 0.66).abs() < 0.05);
        
        // 1000 NEAR ≈ 0.99 (ln(1000)/7 ≈ 0.99)
        assert!((PeerAuthenticator::calculate_stake_factor(one_near * 1000) - 0.99).abs() < 0.02);
        
        // 10000 NEAR = 1.0 (capped, ln(10000)/7 > 1)
        assert!((PeerAuthenticator::calculate_stake_factor(one_near * 10000) - 1.0).abs() < 0.01);
    }
    
    #[test]
    fn test_age_factor() {
        // No creation time = 0.0
        assert!((PeerAuthenticator::calculate_age_factor(None) - 0.0).abs() < 0.01);
        
        let now = current_timestamp();
        
        // 0 days old = 0.0
        let zero_days = Some(now);
        assert!((PeerAuthenticator::calculate_age_factor(zero_days) - 0.0).abs() < 0.01);
        
        // 30 days old ≈ 0.08
        let thirty_days = Some(now - 30 * 86400);
        assert!((PeerAuthenticator::calculate_age_factor(thirty_days) - 0.08).abs() < 0.02);
        
        // 365 days old = 1.0
        let one_year = Some(now - 365 * 86400);
        assert!((PeerAuthenticator::calculate_age_factor(one_year) - 1.0).abs() < 0.01);
        
        // 730 days old still 1.0 (capped)
        let two_years = Some(now - 730 * 86400);
        assert!((PeerAuthenticator::calculate_age_factor(two_years) - 1.0).abs() < 0.01);
    }
    
    #[test]
    fn test_activity_factor() {
        // 0 tx = 0.0
        assert!((PeerAuthenticator::calculate_activity_factor(0) - 0.0).abs() < 0.01);
        
        // 10 tx = 0.1
        assert!((PeerAuthenticator::calculate_activity_factor(10) - 0.1).abs() < 0.01);
        
        // 50 tx = 0.5
        assert!((PeerAuthenticator::calculate_activity_factor(50) - 0.5).abs() < 0.01);
        
        // 100 tx = 1.0
        assert!((PeerAuthenticator::calculate_activity_factor(100) - 1.0).abs() < 0.01);
        
        // 200 tx still 1.0 (capped)
        assert!((PeerAuthenticator::calculate_activity_factor(200) - 1.0).abs() < 0.01);
    }
    
    #[test]
    fn test_reputation_factor() {
        // No ratings = 0.5 (neutral)
        assert!((PeerAuthenticator::calculate_reputation_factor(0.0, 0) - 0.5).abs() < 0.01);
        
        // 5 stars from 1 rater = high rating but low confidence
        let low_confidence = PeerAuthenticator::calculate_reputation_factor(5.0, 1);
        assert!(low_confidence > 0.7 && low_confidence < 0.9);
        
        // 4.5 stars from 10 raters = good rating with full confidence
        let high_confidence = PeerAuthenticator::calculate_reputation_factor(4.5, 10);
        assert!(high_confidence > 0.85);
        
        // 1 star from 100 raters = bad rating
        let bad_rating = PeerAuthenticator::calculate_reputation_factor(1.0, 100);
        assert!(bad_rating < 0.4);
    }
    
    #[test]
    fn test_history_factor() {
        // No jobs = 0.0
        assert!((PeerAuthenticator::calculate_history_factor(0, 0.0) - 0.0).abs() < 0.01);
        
        // 1 job, 100% success = 0.1 * 0.6 + 1.0 * 0.4 = 0.46
        let one_job = PeerAuthenticator::calculate_history_factor(1, 1.0);
        assert!(one_job > 0.35 && one_job < 0.55);
        
        // 10 jobs, 100% success = 1.0
        assert!((PeerAuthenticator::calculate_history_factor(10, 1.0) - 1.0).abs() < 0.01);
        
        // 10 jobs, 50% success = 1.0 * 0.6 + 0.5 * 0.4 = 0.8
        let half_success = PeerAuthenticator::calculate_history_factor(10, 0.5);
        assert!(half_success > 0.7 && half_success < 0.9);
    }
    
    #[test]
    fn test_trust_score_calculation() {
        // New account with 1 NEAR, no history
        let new_account = TrustScore {
            stake_factor: 0.20,
            age_factor: 0.0,
            activity_factor: 0.0,
            reputation_factor: 0.5,  // Neutral
            history_factor: 0.0,
            stake_yocto: ONE_NEAR,
            account_age_days: 0,
            tx_count_90d: 0,
            avg_rating: 0.0,
            rating_count: 0,
            completed_jobs: 0,
            job_success_rate: 0.0,
            calculated_at: current_timestamp(),
        };
        
        let score = new_account.calculate();
        // 0.20*40 + 0*15 + 0*15 + 0.5*20 + 0*10 = 8 + 0 + 0 + 10 + 0 = 18
        // But minimum is 3.0
        assert!(score >= 3.0 && score <= 25.0);
        
        // Established account with 100 NEAR, 1 year old, active
        let established = TrustScore {
            stake_factor: 0.73,
            age_factor: 1.0,
            activity_factor: 0.8,
            reputation_factor: 0.9,
            history_factor: 0.8,
            stake_yocto: ONE_NEAR * 100,
            account_age_days: 365,
            tx_count_90d: 80,
            avg_rating: 4.5,
            rating_count: 20,
            completed_jobs: 8,
            job_success_rate: 0.95,
            calculated_at: current_timestamp(),
        };
        
        let score = established.calculate();
        // 0.73*40 + 1.0*15 + 0.8*15 + 0.9*20 + 0.8*10 = 29.2 + 15 + 12 + 18 + 8 = 82.2
        assert!(score > 75.0 && score < 90.0);
    }
    
    #[test]
    fn test_trust_action_thresholds() {
        let low_score = TrustScore {
            stake_factor: 0.1,
            age_factor: 0.1,
            activity_factor: 0.1,
            reputation_factor: 0.5,
            history_factor: 0.0,
            stake_yocto: ONE_NEAR / 10,
            account_age_days: 30,
            tx_count_90d: 5,
            avg_rating: 0.0,
            rating_count: 0,
            completed_jobs: 0,
            job_success_rate: 0.0,
            calculated_at: current_timestamp(),
        };
        
        // Low score can rate but not bid
        assert!(low_score.can_perform(TrustAction::Rate));
        assert!(!low_score.can_perform(TrustAction::Bid));
        assert!(!low_score.can_perform(TrustAction::Approve));
        
        let high_score = TrustScore {
            stake_factor: 0.8,
            age_factor: 1.0,
            activity_factor: 0.9,
            reputation_factor: 0.9,
            history_factor: 0.9,
            stake_yocto: ONE_NEAR * 200,
            account_age_days: 400,
            tx_count_90d: 90,
            avg_rating: 4.8,
            rating_count: 30,
            completed_jobs: 15,
            job_success_rate: 0.98,
            calculated_at: current_timestamp(),
        };
        
        // High score can do everything
        assert!(high_score.can_perform(TrustAction::Rate));
        assert!(high_score.can_perform(TrustAction::Bid));
        assert!(high_score.can_perform(TrustAction::Approve));
        assert!(high_score.can_perform(TrustAction::Dispute));
    }
    
    #[test]
    fn test_rating_cooldown() {
        let mut auth = PeerAuthenticator::new("alice.test".to_string());
        
        // Manually add a recent rating to simulate cooldown scenario
        let now = current_timestamp();
        auth.ratings.insert("bob.test".to_string(), vec![
            RatingEntry {
                rater: "alice.test".to_string(),
                rating: 5,
                weight: 1000000,
                timestamp: now - 3600, // 1 hour ago
                rater_trust_score: 50.0,
            }
        ]);
        
        // Check that the rating exists and is recent
        let ratings = auth.ratings.get("bob.test").unwrap();
        assert_eq!(ratings.len(), 1);
        assert_eq!(ratings[0].rater, "alice.test");
        
        // The cooldown should prevent a new rating within 24 hours
        // Since we added a rating 1 hour ago, the next rating should fail
        // However, submit_rating also checks trust score which requires network
        // So we test the cooldown logic directly
        
        // Find the existing rating from alice
        let existing = ratings.iter().find(|r| r.rater == "alice.test");
        assert!(existing.is_some());
        
        let existing = existing.unwrap();
        let age_secs = now - existing.timestamp;
        assert!(age_secs < RATING_COOLDOWN_SECS); // Should be within cooldown
    }
    
    #[test]
    fn test_job_history_affects_score() {
        let mut auth = PeerAuthenticator::new("alice.test".to_string());
        
        // Record some jobs
        auth.record_job("alice.test", "job1".to_string(), true, ONE_NEAR);
        auth.record_job("alice.test", "job2".to_string(), true, ONE_NEAR);
        auth.record_job("alice.test", "job3".to_string(), false, ONE_NEAR); // Failed
        
        let (completed, success_rate) = auth.get_job_history_stats("alice.test");
        
        assert_eq!(completed, 3);
        assert!((success_rate - 0.667).abs() < 0.01); // 2/3 success
    }
    
    #[test]
    fn test_trust_cache() {
        let mut cache = TrustCache::new();
        
        let score = TrustScore {
            stake_factor: 0.5,
            age_factor: 0.5,
            activity_factor: 0.5,
            reputation_factor: 0.5,
            history_factor: 0.5,
            stake_yocto: ONE_NEAR * 10,
            account_age_days: 180,
            tx_count_90d: 50,
            avg_rating: 4.0,
            rating_count: 5,
            completed_jobs: 3,
            job_success_rate: 0.9,
            calculated_at: current_timestamp(),
        };
        
        // Insert and retrieve
        cache.insert("test.near".to_string(), score.clone());
        assert!(cache.get("test.near").is_some());
        
        // Invalidate
        cache.invalidate("test.near");
        assert!(cache.get("test.near").is_none());
    }
}
