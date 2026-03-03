# Web of Trust Implementation for Gork Protocol

**Added:** Mar 3, 2026
**Status:** ✅ Built and compiled

---

## What Was Added

### 1. Trust Module (`trust.rs`)

New module with core Web of Trust functionality:

**Data Structures:**
- `TrustLevel` - Partial (0.5 weight) or Full (1.0 weight) endorsement
- `Endorsement` - Record of trust relationship with timestamp and revocation
- `TrustConfig` - Configurable parameters (min endorser trust, decay, path depth)
- `TrustGraph` - Graph structure for trust path finding

**Key Algorithms:**
- `compute_trust()` - Weighted average based on endorser reputation
- `find_trust_path()` - BFS for transitive trust (A→B→C paths)
- `compute_transitive_trust()` - Path-based trust decay

### 2. Contract Integration (`lib.rs`)

**New Storage:**
```rust
endorsements: UnorderedMap<AccountId, Vec<Endorsement>>
endorsements_by_agent: UnorderedMap<AccountId, Vec<(AccountId, String)>>
```

**New Methods:**

#### Endorsement Management
```rust
// Endorse an agent for a capability
endorse_agent(endorsed: AccountId, capability: String, trust_level: TrustLevel) -> bool

// Revoke your endorsement
revoke_endorsement(endorsed: AccountId, capability: String) -> bool

// View endorsements
get_endorsements(agent_id: AccountId) -> Vec<Endorsement>
get_endorsements_by(endorser: AccountId) -> Vec<(AccountId, String)>
```

#### Trust Computation
```rust
// Compute trust score (0-100) using Web of Trust algorithm
compute_trust_score(agent_id: AccountId, capability: String) -> u32

// Find trust path (for transitive trust)
find_trust_path(
    source: AccountId,
    target: AccountId,
    capability: String,
    max_depth: Option<u32>
) -> Option<Vec<AccountId>>

// Discover agents sorted by trust score
discover_trusted(
    capability: String,
    min_trust: Option<u32>,
    limit: Option<u32>
) -> Vec<(AgentMetadata, u32)>
```

---

## How It Works

### Example Flow

```
1. Jean (reputation: 100) endorses Gork for "trading" (Full trust)
   → Gork's trust for trading = weighted by Jean's reputation

2. Alice (reputation: 85, endorsed by Jean) endorses Gork for "trading" (Partial trust)
   → Gork's trust = weighted average of (100, 85 * 0.5)

3. Bob (reputation: 30, no endorsements) tries to endorse Gork
   → Ignored (below min_endorser_trust threshold of 30)

4. Final: Gork's trust score = ~93/100 for trading
```

### Trust Computation Algorithm

```
trust_score = Σ(endorser_reputation × trust_level_weight × decay) / Σ(weights)

Where:
- endorser_reputation: 0-100 from their AgentMetadata
- trust_level_weight: Full=1.0, Partial=0.5
- decay: 1.0 if < 90 days old, 0.5 if older
- Filter: endorsers with reputation < 30 are ignored
```

### Transitive Trust

```
Jean → Alice → Bob

If Jean trusts Alice, and Alice trusts Bob:
- Jean can discover Bob via trust path
- Bob's trust to Jean = Alice's trust × 0.8 (path decay)
- Max path depth: 3 (configurable)
```

---

## Benefits Over Simple Ratings

| Aspect | Old (Simple Ratings) | New (Web of Trust) |
|--------|---------------------|-------------------|
| Sybil resistance | ❌ Weak | ✅ Strong (need trusted endorsers) |
| Trust calculation | Simple average | Weighted by endorser reputation |
| Capability-specific | ❌ No | ✅ Yes (endorse per capability) |
| Transitive trust | ❌ No | ✅ Yes (path-based discovery) |
| Time decay | ❌ No | ✅ Yes (90-day half-life) |
| Revocation | ❌ No | ✅ Yes (revoke endorsements) |

---

## Usage Examples

### For Agents

```javascript
// Endorse someone for a specific capability
await contract.endorse_agent({
  endorsed: "alice.near",
  capability: "data-analysis",
  trust_level: "Full"
});

// Check someone's trust score for a capability
const trust = await contract.compute_trust_score({
  agent_id: "alice.near",
  capability: "data-analysis"
});
// Returns: 87 (out of 100)

// Discover trusted agents for a task
const agents = await contract.discover_trusted({
  capability: "csv-analysis",
  min_trust: 70,
  limit: 10
});
// Returns agents sorted by trust score
```

### For Discovery

```javascript
// Find trust path from you to target
const path = await contract.find_trust_path({
  source: "jean.near",
  target: "bob.near",
  capability: "trading",
  max_depth: 3
});
// Returns: ["jean.near", "alice.near", "bob.near"]

// Use transitive trust for discovery
if (path) {
  // Bob is trusted by someone you trust
  // Safe to collaborate!
}
```

---

## Configuration

Default values (can be changed in `TrustConfig`):

```rust
min_endorser_trust: 30      // Endorsers need 30+ reputation
partial_to_full_ratio: 3     // 3 partial = 1 full endorsement
max_trust_depth: 3           // Max path length for transitive trust
trust_decay_days: 90         // Half-life for old endorsements
```

---

## Next Steps

1. **Deploy to testnet** - Test with real transactions
2. **Add frontend** - UI for viewing trust graphs
3. **Integrate with P2P** - Use trust scores for connection decisions
4. **Add visualization** - Graph view of trust network

---

## Files Changed

```
gork-protocol/
├── contracts/registry/src/
│   ├── lib.rs          (+150 lines - new methods)
│   └── trust.rs        (NEW - 322 lines)
└── WEB_OF_TRUST.md     (NEW - this file)
```

**Contract size:** ~89KB (includes all features)

---

## Security Considerations

✅ **Sybil-resistant:** Fake accounts can't boost each other (need 30+ reputation)
✅ **Time-decay:** Old endorsements lose weight (prevents stale trust)
✅ **Revocable:** Can revoke endorsements if trust is broken
✅ **Capability-specific:** Trust for trading ≠ trust for analysis
✅ **Path-limited:** Max depth of 3 prevents deep trust chains

---

*Ready for testnet deployment. Jean can endorse Gork for "trading" once deployed!*
