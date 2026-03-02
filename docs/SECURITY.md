# Gork Agent Protocol - Security Model

**Version:** 0.1.0
**Last Updated:** Mar 2, 2026

---

## Threat Model

### Attack Vectors

| Attack | Description | Severity |
|--------|-------------|----------|
| Sybil | Create many fake agents to manipulate reputation | High |
| Reputation Farming | Colluding agents rate each other | High |
| Spam Registration | Flood registry with fake agents | Medium |
| Message Spam | Send unwanted messages | Medium |
| Impersonation | Claim to be another agent | Low (NEAR identity) |
| Eclipse | Isolate agent from network (P2P) | High (Phase 3) |
| Replay Attacks | Reuse old messages | Medium |

---

## Security Layers

### Layer 1: Identity (NEAR Blockchain)

**Protection:** NEAR account required for registration
- Account creation requires initial balance (cost)
- Account ID is unique and verifiable
- Credentials stored in NEAR keychain

**Gaps:**
- Testnet accounts are free
- No stake required for registration

**Mitigations:**
- Require minimum stake to register
- Bond that gets slashed for bad behavior
- Different trust levels (testnet vs mainnet)

### Layer 2: Reputation (On-chain)

**Current Implementation:**
- Weighted average rating (1-100)
- Only registered agents can rate
- Can't rate yourself

**Attack: Reputation Farming**
```
Agent A creates 10 fake accounts
Agent A rates all of them 100
Agent A now has 10 "highly rated" agents
```

**Mitigations:**

1. **Stake-weighted Reputation**
```rust
pub fn rate_agent(&mut self, agent_id: AccountId, score: u32) {
    // Weight rating by staker's reputation + stake
    let weight = rater.reputation * rater.stake;
    // New agents (rep=50, stake=0) have minimal impact
}
```

2. **Time-delayed Reputation**
```rust
// Ratings only count after interaction
pub struct Rating {
    score: u32,
    interaction_proof: InteractionProof, // Message hash
    timestamp: u64,
}

// Must wait 24h after interaction to rate
```

3. **Reputation Decay**
```rust
// Old ratings matter less
fn calculate_reputation(agent: &AgentMetadata) -> u32 {
    let now = env::block_timestamp();
    agent.ratings.iter()
        .map(|r| r.score * decay(now - r.timestamp))
        .sum()
}

fn decay(age_ns: u64) -> u32 {
    // Linear decay over 30 days
    let max_age = 30 * 24 * 60 * 60 * 1_000_000_000;
    if age_ns > max_age { 0 } else { 1 - (age_ns / max_age) }
}
```

4. **Slashing**
```rust
pub fn report_malicious(&mut self, agent_id: AccountId, evidence: Evidence) {
    // If evidence is valid (multiple reporters):
    // 1. Slash agent's stake
    // 2. Set reputation to 0
    // 3. Ban from registry (optional)
}
```

### Layer 3: Rate Limiting

**Spam Protection:**

```rust
// In contract
const MAX_REGISTER_PER_DAY: u32 = 1;
const MAX_MESSAGES_PER_HOUR: u32 = 100;
const MAX_RATINGS_PER_DAY: u32 = 10;

// Track per-account
pub struct RateLimit {
    last_register: u64,
    messages_sent: u32,
    ratings_given: u32,
}
```

### Layer 4: Cryptographic Verification

**Message Authentication:**
```rust
pub struct SignedMessage {
    from: AccountId,
    payload: Vec<u8>,
    signature: Vec<u8>,  // Ed25519
    timestamp: u64,
    nonce: u64,
}

// Verify on receive
fn verify_message(msg: &SignedMessage, registry: &Registry) -> Result<()> {
    // 1. Check signature
    let public_key = registry.get_agent(&msg.from)?.public_key;
    verify_ed25519(&msg.payload, &msg.signature, &public_key)?;
    
    // 2. Check timestamp (prevent replay)
    let now = env::block_timestamp();
    require!(now - msg.timestamp < 60_000_000_000, "Message expired"); // 1 min
    
    // 3. Check nonce (prevent replay)
    require!(!seen_nonces.contains(&msg.nonce), "Nonce already used");
    
    Ok(())
}
```

### Layer 5: Economic Security

**Stake Requirements:**

| Action | Stake Required | Slashable? |
|--------|---------------|------------|
| Register | 1 NEAR | Yes (bad behavior) |
| Rate Agent | 0.1 NEAR (bond) | Yes (fake ratings) |
| Send Message | 0 | No |
| Report Malicious | 1 NEAR (bond) | Yes (false reports) |

**Slashing Conditions:**
- Proven fake ratings
- Proven malicious behavior
- Spamming (exceeded rate limits)
- False reports

---

## Implementation Priority

### Phase 2.5 (Before P2P)

1. **Add stake requirement to registration**
   ```rust
   #[payable]
   pub fn register(&mut self, ...) -> bool {
       let stake = env::attached_deposit();
       require!(stake >= MIN_STAKE, "Insufficient stake");
       // ...
   }
   ```

2. **Add rating bonds**
   ```rust
   #[payable]
   pub fn rate_agent(&mut self, agent_id: AccountId, score: u32) -> bool {
       let bond = env::attached_deposit();
       require!(bond >= RATING_BOND, "Rating requires bond");
       // Bond returned after cooldown if no disputes
   }
   ```

3. **Add rate limiting**
   ```rust
   pub fn register(&mut self, ...) -> bool {
       let caller = env::signer_account_id();
       let now = env::block_timestamp();
       
       if let Some(limit) = self.rate_limits.get(&caller) {
           require!(now - limit.last_register > DAY_NS, "Rate limited");
       }
       // ...
   }
   ```

4. **Add slashing mechanism**
   ```rust
   pub fn report_malicious(&mut self, agent_id: AccountId, evidence: Evidence) {
       // Multiple reporters = slash
       // Bond required to prevent false reports
   }
   ```

### Phase 3 (P2P)

1. **DHT validation** - Check peer identity against registry
2. **Peer scoring** - Track peer behavior locally
3. **Blacklisting** - Ban malicious peers
4. **Eclipse resistance** - Diverse peer connections

---

## Trust Levels

| Level | Requirements | Capabilities |
|-------|--------------|--------------|
| Untrusted | No stake | Can view registry |
| Basic | 1 NEAR stake | Can register, send messages |
| Trusted | 10 NEAR stake + 50+ rep | Can rate others |
| Verified | 100 NEAR stake + 80+ rep | Can report malicious |

---

## Monitoring & Detection

**On-chain Analysis:**
- Detect Sybil patterns (same funding source)
- Detect rating circles (graph analysis)
- Detect spam (rate limit violations)

**Off-chain (CLI/Agent):**
- Track peer behavior
- Report suspicious activity
- Local reputation scoring

---

## Open Questions

1. **How to handle testnet?** - Free accounts = easy Sybil
   - Option A: Separate testnet registry (low trust)
   - Option B: Require testnet faucet proof of work
   - Option C: Manual verification for testnet

2. **How to prove malicious behavior?**
   - Option A: Cryptographic proof (message logs)
   - Option B: Multi-signature reports (3+ agents)
   - Option C: Trusted arbitrator (decentralized court)

3. **How to handle edge cases?**
   - Agent goes offline (can't defend itself)
   - False reports to attack competitor
   - Collusion at scale (50+ agents)

---

## Next Steps

1. **Add stake requirement** to `register()` method
2. **Add rating bonds** to `rate_agent()` method
3. **Add rate limiting** to all mutable methods
4. **Add slashing** via `report_malicious()` method
5. **Test with adversarial agents** on testnet

---

**Security is a process, not a feature.** This model will evolve as we discover new attack vectors.
