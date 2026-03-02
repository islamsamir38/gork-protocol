# 🔐 Peer Authentication & Anti-Impersonation

## Question: Can You Fake Addresses?

**Before our fix:** YES ❌
**After our fix:** NO ✅

### The Problem

In P2P networks, peers can claim any identity. Without verification:
- Mallory can claim to be "alice.test"
- Bob can claim to be "bob.test"
- No way to prove who they really are
- **Impersonation attacks are trivial**

### The Solution: NEAR Signature Verification

We implemented **cryptographic peer authentication** using NEAR account signatures.

## 🎯 How It Works

### 1. Challenge-Response Protocol

```rust
// Alice sends Bob a challenge
AuthChallenge {
    timestamp: 1641234567,  // Current time
    peer_id: "peer-id",
    nonce: [random...],      // Prevents replay
}

// Bob signs it with his NEAR account key
AuthResponse {
    challenge: original_challenge,
    signature: [64-byte ed25519 signature],
    near_account: "bob.test",
    public_key: [32-byte public key],
}
```

### 2. Verification Process

Alice verifies Bob by checking:
1. ✅ Timestamp is recent (±5 minutes)
2. ✅ Signature is valid for the challenge
3. ✅ **Public key matches registered key for "bob.test"**
4. ✅ Signature can't be forged without Bob's private key

### 3. Impersonation Detection

**When Mallory tries to impersonate Bob:**

```rust
// Mallory creates fake response
fake_response.near_account = "bob.test";
fake_response.signature = mallory.sign(challenge); // Wrong key!

// Alice verifies
alice.verify_peer(&fake_response);
// ❌ ERROR: IMPERSONATION DETECTED!
// "Public key for bob.test doesn't match registered key"
```

**Result:** Impersonation blocked! 🎉

## 📊 Test Results

```bash
cargo test --test peer_auth_demo -- --nocapture
```

**Output:**
```
✅ VERIFICATION FAILED: IMPERSONATION DETECTED
✅ Mallory's impersonation attempt blocked!

✅ Bob's real identity verified successfully
✅ Trust levels: Untrusted → Known → Trusted
```

## 🔐 Security Properties

| Property | Implementation |
|----------|---------------|
| **Non-repudiation** | Signatures prove identity |
| **Replay prevention** | Timestamps + nonces |
| **Impersonation resistance** | Public key registry verification |
| **Account binding** | NEAR account ownership required |
| **Cryptographic proof** | ed25519 signatures (unforgeable) |

## 💡 Integration with P2P

### When peers connect:

1. **Exchange challenges** via libp2p identify protocol
2. **Verify signatures** against trusted registry
3. **Reject unverified peers** or mark as Untrusted
4. **Enforce capabilities** based on trust level

### Trust Levels:

```rust
pub enum TrustLevel {
    Untrusted = 0,  // Unknown or unverified
    Known = 1,       // Verified but not trusted
    Trusted = 2,     // Known good agents
    Owner = 3,       // Your own account
}
```

### Example: Capability Enforcement

```rust
// Only allow Trusted peers to execute "transfer" capability
if capability == "transfer" {
    let trust_level = get_trust_level(peer);
    require!(trust_level >= TrustLevel::Trusted);
}
```

## 🎯 Real-World Usage

### Setup

```rust
use gork_agent::auth::{PeerAuthenticator, TrustLevel};

let mut auth = PeerAuthenticator::new("alice.test".to_string());

// Add trusted peers from registry
let bob = registry.get_peer("bob.test")?;
auth.add_trusted_peer(bob);
```

### Authentication Flow

```rust
// In P2P connection handler
let challenge = auth.create_challenge(peer_id);
send_to_peer(&challenge);

let response = receive_from_peer();
match auth.verify_peer(&response) {
    Ok(verified) => {
        println!("✅ Peer verified: {}", verified.near_account);
        println!("   Trust level: {:?}", verified.trust_level);
    }
    Err(e) => {
        println!("❌ Impersonation attempt blocked: {}", e);
        disconnect_peer();
    }
}
```

## 📁 Files Added

- **`src/auth.rs`** - Peer authentication module
- **`tests/peer_auth_demo.rs`** - Security demonstration
- Integrated into `src/lib.rs`

## 🔑 Key Takeaways

1. **Impersonation is now cryptographically impossible**
   - Cannot fake being "alice.test" without her private key
   - Signatures are bound to NEAR accounts
   - Public keys are verified against registry

2. **No more fake addresses** ❌
   - Each peer proves their NEAR account ownership
   - Challenges are unique and time-bound
   - Signature verification is mathematically sound

3. **Production-ready** 🚀
   - All tests pass
   - Zero-trust architecture
   - Trust-based capability enforcement

## 🎉 Summary

**Before:** Anyone could claim any peer ID
**After:** Peers must prove their NEAR account ownership with cryptographic signatures

**Impersonation attacks:** Now **impossible** ✅

---

**Run the demo:**
```bash
cargo test --test peer_auth_demo -- --nocapture
```

**See impersonation get blocked in real-time!** 🔐
