use near_sdk::{env, AccountId, require};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Trust level for endorsements
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, JsonSchema)]
pub enum TrustLevel {
    Partial,  // "I somewhat trust this agent"
    Full,     // "I fully trust this agent"
}

impl TrustLevel {
    pub fn weight(&self) -> f32 {
        match self {
            TrustLevel::Partial => 0.5,
            TrustLevel::Full => 1.0,
        }
    }
}

/// An endorsement from one agent to another
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, JsonSchema)]
pub struct Endorsement {
    pub endorser: String,  // Changed from AccountId to String for JsonSchema
    pub endorsed: String,  // Changed from AccountId to String for JsonSchema
    pub capability: String,
    pub trust_level: TrustLevel,
    pub timestamp: u64,
    pub revoked: bool,
}

/// Trust configuration
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct TrustConfig {
    /// Minimum trust score required for endorsements to count (0-100)
    pub min_endorser_trust: u32,
    /// Number of partial endorsements needed to equal one full endorsement
    pub partial_to_full_ratio: u32,
    /// Maximum trust path length (transitive trust depth)
    pub max_trust_depth: u32,
    /// Decay factor for trust over time (days)
    pub trust_decay_days: u64,
}

impl Default for TrustConfig {
    fn default() -> Self {
        Self {
            min_endorser_trust: 30,
            partial_to_full_ratio: 3,
            max_trust_depth: 3,
            trust_decay_days: 90,
        }
    }
}

// Note: TrustGraph removed - trust logic implemented directly in lib.rs using UnorderedMap
// to avoid std::collections which are incompatible with NEAR wasm
