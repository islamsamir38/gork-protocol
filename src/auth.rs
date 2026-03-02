//! Peer Authentication Module
//!
//! Provides NEAR signature-based peer identity verification
//! to prevent peer impersonation in P2P networks.

use anyhow::{anyhow, Result};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey, SigningKey};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use reqwest::Client;
use tracing::info;

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
        }
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

    /// Fetch public key from NEAR blockchain via RPC
    pub async fn fetch_near_public_key(&self, account_id: &str) -> Result<Vec<u8>> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "view_access_key",
                "finality": "final",
                "account_id": account_id,
                "public_key": account_id
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

        // Extract public key from result
        let public_key_str = result
            .get("result")
            .and_then(|r| r.get("public_key"))
            .and_then(|k| k.as_str())
            .ok_or_else(|| anyhow!("No public key found in response"))?;

        // NEAR public keys are base58 encoded, but we need the raw bytes
        // For ed25519, we'll decode it and return as bytes
        // The public key format in NEAR is typically base58
        match bs58::decode(public_key_str).into_vec() {
            Ok(key_bytes) if key_bytes.len() == 32 => Ok(key_bytes),
            Ok(_) => Err(anyhow!("Invalid public key length")),
            Err(_) => Err(anyhow!("Invalid base58 public key")),
        }
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
        // Query the NEAR blockchain to get the legitimate public key

        // First check our cache (in case we've already verified them)
        let expected_public_key = if let Some(known_peer) = self.trusted_peers.get(&response.near_account) {
            known_peer.public_key.clone()
        } else {
            // Unknown peer - fetch from NEAR blockchain
            match self.fetch_near_public_key(&response.near_account).await {
                Ok(key) => {
                    // Cache it for future use
                    info!("Fetched public key from NEAR blockchain for {}", response.near_account);
                    key
                }
                Err(e) => {
                    // Account might not exist or network error
                    return Err(anyhow!(
                        "Failed to fetch public key from NEAR blockchain for {}: {}. \
                        Cannot verify identity.",
                        response.near_account, e
                    ));
                }
            }
        };

        // Verify the public key matches
        if expected_public_key != response.public_key {
            return Err(anyhow!(
                "IMPERSONATION DETECTED: Public key for {} doesn't match NEAR blockchain! \
                Expected (from blockchain): {:?}, Got: {:?}",
                response.near_account,
                &expected_public_key[..16],
                &response.public_key[..16]
            ));
        }

        // 4. Create verified peer
        let verified = VerifiedPeer {
            near_account: response.near_account.clone(),
            peer_id: response.challenge.peer_id.clone(),
            public_key: response.public_key.clone(),
            verified_at: now,
            trust_level: TrustLevel::Known, // Default to Known for verified peers
        };

        // 5. Cache the verified peer
        self.trusted_peers.insert(verified.near_account.clone(), verified.clone());

        Ok(verified)
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
        }
    }

    /// Add trusted peer (e.g., from registry)
    pub fn add_trusted_peer(&mut self, peer: VerifiedPeer) {
        self.trusted_peers.insert(peer.near_account.clone(), peer);
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
