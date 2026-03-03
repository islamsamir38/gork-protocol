# Variant C Implementation Plan

**Date:** Mar 3, 2026
**Status:** In Progress

---

## Architecture

### 1. Agent Keypair (Separate from NEAR)
```
Agent generates its own keypair:
- Private key: Stored in ~/.gork-agent (encrypted)
- Public key: Shared in certificate

NEAR key stays in ~/.near-credentials (never leaves)
```

### 2. Registration Flow
```
1. User runs: gork-agent register --account user.testnet
2. Agent generates keypair
3. Calls contract: registry.register_agent(public_key)
4. Contract verifies: signer == user.testnet
5. Contract stores: user.testnet → public_key
6. Agent creates certificate signed by NEAR key
7. Certificate stored locally
```

### 3. Verification Flow
```
Peer receives connection from agent:
1. Agent sends certificate
2. Peer verifies certificate signature (off-chain, fast)
3. Check: certificate.near_account matches claimed account
4. Check: certificate not expired
5. Optional: Query contract if suspicious

All verification happens off-chain (no RPC calls)
```

---

## Contract Changes

### Add to registry/src/lib.rs:

```rust
/// Agent registration (separate from NEAR key)
#[derive(BorshSerialize, BorshDeserialize)]
pub struct AgentRegistration {
    pub public_key: Vec<u8>,
    pub registered_at: u64,
    pub expires_at: u64,
}

// Add to StorageKey enum
EndorsementsByAgent,
AgentRegistrations, // NEW

// Add to AgentRegistry struct
agent_registrations: UnorderedMap<AccountId, AgentRegistration>,

// New methods
pub fn register_agent(&mut self, public_key: Vec<u8>) {
    let account = env::signer_account_id();
    
    self.agent_registrations.insert(&account, &AgentRegistration {
        public_key,
        registered_at: env::block_timestamp(),
        expires_at: env::block_timestamp() + (365 * 24 * 60 * 60 * 1000), // 1 year
    });
    
    env::log(&format!("Agent registered: {}", account));
}

pub fn verify_agent(&self, account_id: AccountId, public_key: Vec<u8>) -> bool {
    self.agent_registrations.get(&account_id)
        .map(|reg| reg.public_key == public_key && reg.expires_at > env::block_timestamp())
        .unwrap_or(false)
}

pub fn revoke_agent(&mut self, account_id: AccountId) {
    let caller = env::signer_account_id();
    require!(caller == account_id, "Can only revoke own registration");
    
    self.agent_registrations.remove(&account_id);
}
```

---

## Agent Changes

### New Command: register

```rust
// In main.rs Commands enum
Register {
    /// NEAR account to register
    #[arg(short, long)]
    account: String,
},

// Implementation
async fn register_agent(account: &str, network: &str) -> Result<()> {
    // 1. Check NEAR credentials exist
    let near_identity = near::NearIdentity::new(account.to_string(), network);
    if !near_identity.has_credentials() {
        return Err(anyhow!("Run 'near login --account-id {}' first", account));
    }
    
    // 2. Generate agent keypair (separate from NEAR)
    let agent_crypto = crypto::MessageCrypto::new()?;
    let agent_public_key = agent_crypto.public_key();
    
    // 3. Register on-chain
    println!("📝 Registering agent on blockchain...");
    near call registry-wot.testnet register_agent \
        '{"public_key": [...]}' \
        --accountId account --networkId network
    
    // 4. Create certificate signed by NEAR key
    let cert = create_certificate(account, agent_public_key, &near_identity)?;
    
    // 5. Store certificate and agent keypair
    storage.save_certificate(&cert)?;
    storage.save_agent_keypair(&agent_crypto)?;
    
    println!("✅ Agent registered!");
    println!("   NEAR account: {}", account);
    println!("   Agent public key: {}", hex::encode(&agent_public_key));
}
```

---

## Certificate Creation

```rust
fn create_certificate(
    near_account: &str,
    agent_public_key: Vec<u8>,
    near_identity: &near::NearIdentity,
) -> Result<AgentCertificate> {
    let mut cert = AgentCertificate::new(
        near_account.to_string(),
        agent_public_key,
        365, // 1 year
    );
    
    // Sign with NEAR private key
    let message = cert.sign_message();
    let near_creds = near_identity.load_credentials()?;
    let signature = sign_with_near_key(&message, &near_creds.private_key)?;
    
    cert.signature = signature;
    
    Ok(cert)
}
```

---

## Verification in P2P Layer

```rust
// When receiving connection
pub fn verify_peer_certificate(&self, cert: &AgentCertificate) -> Result<bool> {
    // 1. Check not expired
    if !cert.is_valid() {
        return Ok(false);
    }
    
    // 2. Verify signature with NEAR public key
    let near_pubkey = self.get_near_public_key(&cert.near_account)?;
    let message = cert.sign_message();
    
    if !verify_signature(&message, &cert.signature, &near_pubkey)? {
        return Ok(false);
    }
    
    // 3. Optional: Check on-chain if suspicious
    if self.is_suspicious(&cert.near_account) {
        let is_registered = self.query_contract_verify(&cert)?;
        if !is_registered {
            return Ok(false);
        }
    }
    
    Ok(true)
}
```

---

## Benefits

✅ **Key separation** - Agent key ≠ NEAR key
✅ **Fast verification** - Off-chain (no RPC)
✅ **On-chain proof** - One-time registration
✅ **Revocation** - Contract can revoke
✅ **Low cost** - One-time ~0.1 NEAR
✅ **Secure** - NEAR key never leaves ~/.near-credentials

---

## Implementation Steps

1. ✅ Create certificate module
2. ⏳ Add registration to contract
3. ⏳ Add register command to agent
4. ⏳ Update verification in P2P layer
5. ⏳ Test end-to-end

---

**Files to modify:**
- `contracts/registry/src/lib.rs` - Add registration methods
- `src/certificate.rs` - Certificate logic (DONE)
- `src/main.rs` - Add register command
- `src/network/mod.rs` - Add verification
- `src/storage/mod.rs` - Store certificates
