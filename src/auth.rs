//! Peer Authentication Module
//!
//! Provides NEAR signature-based peer identity verification
//! to prevent peer impersonation in P2P networks.

use anyhow::{anyhow, Result};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey, SigningKey};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use reqwest::Client;
use tracing::{info, warn};
use std::path::Path;
use rocksdb::{DB, Options};

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
}

/// Peer authentication message type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerAuthMessage {
    Challenge(AuthChallenge),
    Response(AuthResponse),
    Verified(VerifiedPeer),
    Error(String),
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
}
