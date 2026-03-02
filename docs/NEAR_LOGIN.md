# 🔐 NEAR Login & Authentication

## Overview

The Gork Agent Protocol supports **cryptographic proof of NEAR account ownership** to prevent peer impersonation. When you initialize with `--verify-near`, your agent proves it actually owns the NEAR account it's claiming.

## Why NEAR Verification Matters

### ❌ Without Verification (Insecure)

```bash
gork-agent init --account alice.test
```

**Problem:** Anyone can claim any NEAR account ID:
- Mallory can claim to be "alice.test"
- Bob can claim to be "alice.test"
- **No way to prove who you really are**
- Impersonation attacks are trivial

### ✅ With NEAR Verification (Secure)

```bash
gork-agent init --account alice.test --verify-near
```

**Solution:**
- Loads your NEAR credentials from `~/.near-credentials/`
- Verifies account exists on blockchain via RPC
- Uses your **real NEAR private key** for signing
- **Cryptographically proves account ownership**
- Impersonation becomes impossible

---

## 🚀 Quick Start

### Step 1: Install NEAR CLI

```bash
npm install -g near-cli
```

### Step 2: Login to Your NEAR Account

```bash
near login --account-id your-account.testnet
```

This will:
1. Open NEAR wallet in your browser
2. Authorize the NEAR CLI to access your account
3. Save credentials to `~/.near-credentials/testnet/your-account.testnet.json`

### Step 3: Initialize Your Agent with Verification

```bash
gork-agent init --account your-account.testnet --verify-near
```

**Output:**
```
🔐 Verifying NEAR account ownership...
   Account: your-account.testnet
✅ NEAR credentials loaded from: ~/.near-credentials/testnet/your-account.testnet.json
🔍 Verifying account exists on testnet...
✅ Account verified on blockchain

✅ Agent initialized successfully!
   Account: your-account.testnet
   Network: testnet
   Verified: ✅ NEAR account ownership confirmed
```

---

## 🔑 How It Works

### 1. NEAR Credential Loading

When you use `--verify-near`, the agent:

```rust
// Load credentials from NEAR CLI's credential file
let creds_path = "~/.near-credentials/testnet/account.json";
let creds = load_credentials(creds_path)?;

// Extract the private key
let private_key = decode_near_private_key(&creds.private_key)?;

// Use it for signing
let crypto = MessageCrypto::from_keys(&private_key, &private_key)?;
```

### 2. Blockchain Verification

The agent verifies your account exists on the blockchain:

```rust
// Query NEAR RPC
let body = serde_json::json!({
    "jsonrpc": "2.0",
    "method": "query",
    "params": {
        "request_type": "view_account",
        "account_id": "your-account.testnet"
    }
});

// Verify account exists
let account_exists = near_identity.validate_account().await?;
```

### 3. Peer Authentication

When connecting to other agents:

1. **Alice** sends Bob a challenge
2. **Bob** signs it with his NEAR private key
3. **Alice** verifies:
   - ✅ Signature is valid
   - ✅ Public key matches Bob's claimed NEAR account
   - ✅ Bob actually owns that account

**Result:** Cryptographically proven identity! 🔐

---

## 📖 Detailed Examples

### Example 1: Basic Initialization

```bash
# Initialize with NEAR verification
gork-agent init --account alice.near --verify-near
```

### Example 2: With Capabilities

```bash
# Initialize with specific capabilities
gork-agent init --account alice.near --verify-near --capabilities "chat,payment,file-transfer"
```

### Example 3: Testnet vs Mainnet

```bash
# Testnet (default)
gork-agent init --account alice.testnet --verify-near

# Mainnet
gork-agent init --account alice.near --verify-near --network mainnet
```

### Example 4: Development/Testing (Insecure)

⚠️ **For testing only - never use in production!**

```bash
# Initialize without verification (insecure!)
gork-agent init --account test.near
```

Or use a specific private key:

```bash
gork-agent init --account test.near --private-key <base58-key>
```

---

## 🔒 Security Properties

| Property | Without Verification | With NEAR Verification |
|----------|---------------------|------------------------|
| **Identity Proof** | ❌ None | ✅ Cryptographic signature |
| **Impersonation Resistance** | ❌ Vulnerable | ✅ Protected |
| **Blockchain Verification** | ❌ None | ✅ RPC verified |
| **Key Management** | ⚠️  Local keypair | ✅ NEAR wallet credentials |
| **Trust Level** | Untrusted | Trusted |

---

## 📁 Credential Locations

NEAR CLI stores credentials in:

```
~/.near-credentials/
├── testnet/
│   ├── account1.testnet.json
│   └── account2.testnet.json
└── mainnet/
    ├── account1.near.json
    └── account2.near.json
```

Each credential file contains:

```json
{
  "account_id": "account.testnet",
  "public_key": "ed25519:...",
  "private_key": "ed25519:..."
}
```

---

## 🛠️ Troubleshooting

### Error: "NEAR credentials not found"

**Cause:** NEAR CLI is not logged in

**Solution:**
```bash
near login --account-id your-account.testnet
```

### Error: "Account does not exist on testnet"

**Cause:** Account ID is misspelled or doesn't exist

**Solution:**
- Verify account ID is correct
- Check if account exists: https://explorer.testnet.near.org/accounts/YOUR-ACCOUNT
- Create account if needed: https://wallet.testnet.near.org/

### Error: "Invalid private key format"

**Cause:** Credential file is corrupted

**Solution:**
```bash
# Re-login with NEAR CLI
near login --account-id your-account.testnet

# Then retry
gork-agent init --account your-account.testnet --verify-near
```

---

## 🔐 Best Practices

### 1. **Always Use NEAR Verification in Production**

```bash
# ✅ Good
gork-agent init --account alice.near --verify-near

# ❌ Bad (insecure)
gork-agent init --account alice.near
```

### 2. **Protect Your Credential Files**

```bash
# Ensure only you can read credentials
chmod 600 ~/.near-credentials/*/*.json

# Never share these files!
```

### 3. **Use Separate Accounts for Different Environments**

- **Testnet:** `your-agent.testnet`
- **Mainnet:** `your-agent.near`

### 4. **Backup Your Credentials**

```bash
# Backup NEAR credentials
cp -r ~/.near-credentials ~/near-credentials-backup

# Keep backup secure and encrypted!
```

---

## 🎯 Integration with P2P

When agents connect via P2P:

1. **Exchange challenges** via libp2p identify protocol
2. **Verify signatures** against NEAR blockchain
3. **Reject unverified peers** or mark as Untrusted
4. **Enforce capabilities** based on trust level

### Trust Levels

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

---

## 📚 Related Documentation

- [Peer Authentication](PEER_AUTHENTICATION.md) - How peers verify each other
- [P2P Networking](P2P_NETWORKING.md) - libp2p integration
- [Security Architecture](SECURITY.md) - Overall security design

---

## 🎉 Summary

**With NEAR verification:**
- ✅ **Proves you own your NEAR account**
- ✅ **Prevents impersonation attacks**
- ✅ **Enables trusted P2P communication**
- ✅ **Cryptographically secure identity**

**Without NEAR verification:**
- ⚠️  **Anyone can claim any account**
- ⚠️  **No identity proof**
- ⚠️  **Vulnerable to impersonation**
- ⚠️  **For testing only**

**Always use `--verify-near` in production!** 🔐
