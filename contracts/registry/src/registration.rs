use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Agent registration for Variant C (certificate-based security)
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, JsonSchema)]
pub struct AgentRegistration {
    /// Agent's P2P public key (separate from NEAR key)
    pub public_key: Vec<u8>,
    /// When registration was created (milliseconds)
    pub registered_at: u64,
    /// When registration expires (milliseconds)
    pub expires_at: u64,
}

impl AgentRegistration {
    /// Create new registration (1 year validity)
    pub fn new(public_key: Vec<u8>) -> Self {
        use near_sdk::env;
        let now = env::block_timestamp();
        let year_in_ms = 365 * 24 * 60 * 60 * 1000;
        
        Self {
            public_key,
            registered_at: now,
            expires_at: now + year_in_ms,
        }
    }

    /// Check if registration is still valid
    pub fn is_valid(&self) -> bool {
        use near_sdk::env;
        env::block_timestamp() < self.expires_at
    }
}
