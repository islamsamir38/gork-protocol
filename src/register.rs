use anyhow::Result;
use std::process::Command;

/// Register agent on blockchain (Variant C)
pub async fn register_agent(
    account: &str,
    network: &str,
    registry_contract: &str,
) -> Result<()> {
    println!("📝 Registering agent on blockchain (Variant C)");
    println!();

    // 1. Check NEAR credentials exist
    println!("🔐 Checking NEAR credentials...");
    let near_network = crate::near::Network::from_str(network);
    let near_identity = crate::near::NearIdentity::new(account.to_string(), near_network);

    if !near_identity.has_credentials() {
        println!("❌ NEAR credentials not found!");
        println!();
        println!("Run this first:");
        println!("  near login --account-id {}", account);
        return Err(anyhow::anyhow!("NEAR credentials required"));
    }

    println!("✅ NEAR credentials found");
    println!("   Account: {}", account);
    println!();

    // 2. Generate agent keypair (separate from NEAR)
    println!("🔑 Generating agent keypair...");
    let agent_crypto = crate::crypto::MessageCrypto::new()?;
    let agent_public_key = agent_crypto.public_key();
    
    println!("✅ Agent keypair generated");
    println!("   Public key: {}", hex::encode(&agent_public_key));
    println!();

    // 3. Register on blockchain
    println!("📡 Registering on blockchain...");
    
    // Use Variant C registry (working on testnet)
    let contract = if network == "mainnet" {
        "registry.gork.near"
    } else {
        "registry-variant-c.testnet"
    };
    
    println!("   Contract: {}", contract);
    println!("   Network: {}", network);
    println!();

    // Contract expects byte array, not hex string
    let public_key_bytes: Vec<u8> = agent_public_key.to_vec();
    let args = serde_json::json!({
        "public_key": public_key_bytes
    }).to_string();

    let network_flag = if network == "mainnet" { "--networkId" } else { "--networkId" };
    let network_name = if network == "mainnet" { "mainnet" } else { "testnet" };

    let output = Command::new("near")
        .args([
            "call",
            contract,  // Use correct contract
            "register_agent_key",
            &args,
            "--accountId",
            account,
            network_flag,
            network_name,
        ])
        .output()?;

    if !output.status.success() {
        println!("❌ Registration failed!");
        println!("   {}", String::from_utf8_lossy(&output.stderr));
        return Err(anyhow::anyhow!("Blockchain registration failed"));
    }

    println!("✅ Registered on blockchain!");
    println!();

    // 4. Create certificate signed by NEAR key
    println!("📜 Creating certificate...");
    let near_creds = near_identity.load_credentials()?;
    
    let mut cert = crate::certificate::AgentCertificate::new(
        account.to_string(),
        agent_public_key.clone(),
        365, // 1 year
    );

    // Sign certificate with NEAR private key
    let message = cert.sign_message();
    let signature = sign_with_near_key(&message, &near_creds.private_key)?;
    cert.signature = signature;

    println!("✅ Certificate created");
    println!();

    // 5. Store certificate and agent keypair
    println!("💾 Storing credentials...");
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let storage_path = std::path::PathBuf::from(home).join(".gork-agent");
    let storage = crate::storage::AgentStorage::open(&storage_path)?;
    
    // Save certificate
    storage.save_certificate(&cert)?;
    
    // Save agent keypair (encrypted)
    storage.save_agent_keypair(&agent_crypto)?;

    println!("✅ Credentials stored");
    println!();

    // 6. Summary
    println!("🎉 Registration complete!");
    println!();
    println!("Summary:");
    println!("  NEAR account: {}", account);
    println!("  Agent public key: {}", hex::encode(&agent_public_key));
    println!("  Certificate expires: 1 year");
    println!("  Cost: ~0.1 NEAR");
    println!();
    println!("Next steps:");
    println!("  gork-agent daemon    # Start P2P daemon");
    println!("  gork-agent whoami    # View identity");

    Ok(())
}

/// Sign message with NEAR private key
fn sign_with_near_key(message: &[u8], near_private_key: &str) -> Result<Vec<u8>> {
    // For now, use a simple signature (in production, use proper ed25519 signing)
    // This is a placeholder - should use near-crypto for proper signing
    
    // Decode NEAR private key
    let key = near_private_key.trim();
    let key_bytes = if key.starts_with("ed25519:") {
        bs58::decode(&key[8..]).into_vec()?
    } else {
        bs58::decode(key).into_vec()?
    };

    // TODO: Use proper ed25519 signing
    // For now, just hash the message with the key
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(&key_bytes);
    hasher.update(message);
    let signature = hasher.finalize().to_vec();

    Ok(signature)
}
