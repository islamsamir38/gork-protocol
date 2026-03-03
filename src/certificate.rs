// Agent Registration and Verification (Variant C)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Agent certificate (off-chain, signed by NEAR key)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCertificate {
    /// NEAR account ID
    pub near_account: String,
    /// Agent's P2P public key (separate from NEAR key)
    pub agent_public_key: Vec<u8>,
    /// When certificate was issued
    pub issued_at: i64,
    /// When certificate expires
    pub expires_at: i64,
    /// Signature from NEAR private key
    pub signature: Vec<u8>,
}

impl AgentCertificate {
    /// Create new certificate (called during registration)
    pub fn new(
        near_account: String,
        agent_public_key: Vec<u8>,
        expires_in_days: i64,
    ) -> Self {
        let now = Utc::now().timestamp();
        Self {
            near_account,
            agent_public_key,
            issued_at: now,
            expires_at: now + (expires_in_days * 24 * 60 * 60),
            signature: Vec::new(), // Will be signed by NEAR key
        }
    }

    /// Check if certificate is still valid
    pub fn is_valid(&self) -> bool {
        let now = Utc::now().timestamp();
        now < self.expires_at
    }

    /// Create message to sign (what gets signed by NEAR key)
    pub fn sign_message(&self) -> Vec<u8> {
        format!(
            "{}:{}:{}:{}",
            self.near_account,
            hex::encode(&self.agent_public_key),
            self.issued_at,
            self.expires_at
        ).into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certificate_creation() {
        let cert = AgentCertificate::new(
            "user.testnet".to_string(),
            vec![1, 2, 3, 4],
            365,
        );
        
        assert_eq!(cert.near_account, "user.testnet");
        assert!(cert.is_valid());
    }

    #[test]
    fn test_certificate_expiry() {
        let mut cert = AgentCertificate::new(
            "user.testnet".to_string(),
            vec![1, 2, 3, 4],
            365,
        );
        
        // Set expiry in past
        cert.expires_at = Utc::now().timestamp() - 1000;
        
        assert!(!cert.is_valid());
    }
}
