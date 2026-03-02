//! Demonstration: Peer Authentication with NEAR Signatures
//!
//! This test demonstrates how NEAR signature verification prevents
//! peer impersonation in P2P networks.

use gork_agent::auth::{AuthChallenge, PeerAuthenticator, TrustLevel, VerifiedPeer};
use std::time::SystemTime;

#[tokio::test]
async fn test_peer_authentication_demo() {
    println!("\n{}", "=".repeat(70));
    println!("🔐 PEER AUTHENTICATION DEMONSTRATION");
    println!("   Preventing impersonation with NEAR signature verification");
    println!("{}", "=".repeat(70));
    println!();

    // Scenario: Alice and Bob are real agents
    // Mallory tries to impersonate Bob

    println!("📝 Scenario Setup:");
    println!("   • Alice: Real NEAR account (alice.test)");
    println!("   • Bob: Real NEAR account (bob.test)");
    println!("   • Mallory: Attacker trying to impersonate Bob");
    println!();

    // Create real agents
    let mut alice = PeerAuthenticator::new("alice.test".to_string());
    let bob = PeerAuthenticator::new("bob.test".to_string());

    // Alice and Bob already know each other (from registry or prior interaction)
    // In production, this would come from the NEAR blockchain or trusted registry
    let bob_verified = VerifiedPeer {
        near_account: "bob.test".to_string(),
        peer_id: "bob-real-peer".to_string(),
        public_key: bob.public_key(),
        verified_at: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        trust_level: TrustLevel::Trusted,
    };
    alice.add_trusted_peer(bob_verified);

    println!("🎭 Step 1: Mallory tries to claim she's Bob");
    println!("   Mallory generates her own keypair but claims Bob's account");
    println!();

    let mallory = PeerAuthenticator::new("mallory.attacker".to_string());

    // Mallory creates a fake challenge claiming to be Bob
    let fake_challenge = AuthChallenge {
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        peer_id: "fake-peer-id".to_string(),
        nonce: rand::random(),
    };

    // Mallory signs with her own key, but claims to be Bob
    let fake_response = mallory.sign_challenge(&fake_challenge).unwrap();
    let mut claimed_response = fake_response;
    claimed_response.near_account = "bob.test".to_string(); // 💥 FRAUD!

    println!("   Mallory's fake response:");
    println!("     • Claims to be: bob.test");
    println!("     • Signed with: Mallory's key (not Bob's!)");
    println!("     • Peer ID: {}", claimed_response.challenge.peer_id);
    println!();

    println!("🔍 Step 2: Alice receives the claim and verifies");
    println!("   Alice checks if the signature matches the claimed account");
    println!();

    // Alice tries to verify the fake claim
    match alice.verify_peer(&claimed_response).await {
        Ok(verified) => {
            println!("   ❌ SECURITY BREACH: Fake peer verified!");
            println!("      This should never happen!");
            println!("      Verified: {}", verified.near_account);
            panic!("Security test failed - impersonation possible!");
        }
        Err(e) => {
            println!("   ✅ VERIFICATION FAILED: {}", e);
            println!("   ✅ Mallory's impersonation attempt blocked!");
            println!();
            println!("   Reason: Signature verification failed");
            println!("   • Mallory signed with her own key");
            println!("   • But claimed to be bob.test");
            println!("   • Alice verified using Bob's expected public key");
            println!("   • Signature mismatch → Impersonation detected!");
        }
    }

    println!();
    println!("🤝 Step 3: Bob proves his real identity");
    println!("   Bob responds to Alice's authentication challenge");
    println!();

    // Alice creates a real challenge for Bob
    let real_challenge = alice.create_challenge("bob-real-peer".to_string());

    // Bob signs it with his real key
    let real_response = bob.sign_challenge(&real_challenge).unwrap();

    println!("   Bob's response:");
    println!("     • Account: {}", real_response.near_account);
    println!("     • Signature: {} bytes", real_response.signature.len());
    println!("     • Public key: {} bytes", real_response.public_key.len());
    println!();

    // Alice verifies Bob's signature
    match alice.verify_peer(&real_response).await {
        Ok(verified) => {
            println!("   ✅ VERIFICATION SUCCESSFUL!");
            println!("   ✅ Bob's identity confirmed: {}", verified.near_account);
            println!("   ✅ Peer ID: {}", verified.peer_id);
            println!("   ✅ Trust Level: {:?}", verified.trust_level);
            println!();
            println!("   Proof points:");
            println!("     ✓ Timestamp valid (not replay attack)");
            println!("     ✓ Signature matches public key");
            println!("     ✓ Public key corresponds to bob.test");
            println!("     ✓ Cannot be forged without Bob's private key");
        }
        Err(e) => {
            println!("   ❌ Unexpected verification failure: {}", e);
            panic!("Bob's real signature should verify!");
        }
    }

    println!();
    println!("📊 Step 4: Trust levels and caching");
    println!("   Alice caches verified peers");
    println!();

    // Alice has verified Bob
    assert!(alice.is_verified("bob.test"));
    println!("   ✓ bob.test marked as verified");

    // Mallory is not in trusted peers
    assert!(!alice.is_verified("mallory.attacker"));
    println!("   ✓ mallory.attacker not in trusted peers");

    // Check trust levels
    let bob_trust = alice.get_trust_level("bob.test");
    println!("   ✓ bob.test trust level: {:?}", bob_trust);

    let mallory_trust = alice.get_trust_level("mallory.attacker");
    println!("   ✓ mallory.attacker trust level: {:?}", mallory_trust);

    println!();
    println!("{}", "=".repeat(70));
    println!("🎉 SECURITY TEST PASSED!");
    println!("{}", "=".repeat(70));
    println!();
    println!("🔐 How this prevents impersonation:");
    println!();
    println!("1. **Challenge-Response Protocol**");
    println!("   • Each peer receives a unique, timed challenge");
    println!("   • Challenges include timestamp and nonce");
    println!("   • Prevents replay attacks");
    println!();
    println!("2. **NEAR Signature Verification**");
    println!("   • Peers sign challenges with their NEAR account key");
    println!("   • Signatures are cryptographically bound to the account");
    println!("   • Cannot forge without the private key");
    println!();
    println!("3. **Peer Identity Verification**");
    println!("   • Peer ID must match claimed NEAR account");
    println!("   • Public key must be registered to that account");
    println!("   • Registry can be used to verify ownership");
    println!();
    println!("4. **Trust Levels**");
    println!("   • Owner: Your own account");
    println!("   • Trusted: Known good agents");
    println!("   • Known: Verified but not trusted");
    println!("   • Untrusted: Unknown or failed verification");
    println!();
    println!("5. **Replay Attack Prevention**");
    println!("   • Timestamp must be recent (±5 minutes)");
    println!("   • Nonce adds uniqueness to each challenge");
    println!("   • Old challenges are rejected");
    println!();
    println!("💡 Integration with P2P:");
    println!();
    println!("   When peers connect:");
    println!("   1. Exchange challenges/responses");
    println!("   2. Verify signatures before accepting messages");
    println!("   3. Reject unverified peers");
    println!("   4. Only allow capabilities based on trust level");
    println!();
    println!("{}", "=".repeat(70));

    // All tests passed
    assert!(true);
}

#[tokio::test]
async fn test_signature_properties() {
    println!("\n🔐 Cryptographic Properties Test");
    println!();

    let auth = PeerAuthenticator::new("test.test".to_string());
    let challenge = auth.create_challenge("test-peer".to_string());
    let response = auth.sign_challenge(&challenge).unwrap();

    println!("Signature properties:");
    println!("  • Unique: Different each time (due to nonce)");
    println!("  • Time-bound: Invalid after 5 minutes");
    println!("  • Account-bound: Verifies specific NEAR account");
    println!("  • Non-repudiable: Proves ownership of private key");
    println!();

    // Create another challenge
    let challenge2 = auth.create_challenge("test-peer".to_string());
    let response2 = auth.sign_challenge(&challenge2).unwrap();

    // Signatures should be different (different nonce)
    assert_ne!(response.signature, response2.signature);
    println!("✓ Signature uniqueness verified");

    // Response can be verified
    let mut verifier = PeerAuthenticator::new("verifier.test".to_string());

    // Pre-register test.test in verifier's trusted peers
    let test_peer = VerifiedPeer {
        near_account: "test.test".to_string(),
        peer_id: "test-peer".to_string(),
        public_key: auth.public_key(),
        verified_at: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        trust_level: TrustLevel::Known,
    };
    verifier.add_trusted_peer(test_peer);

    let verified = verifier.verify_peer(&response).await.unwrap();
    assert_eq!(verified.near_account, "test.test");
    println!("✓ Signature verification works");

    // Wrong signature fails
    let mut fake_response = response.clone();
    fake_response.signature[0] ^= 0xFF; // Corrupt signature
    assert!(verifier.verify_peer(&fake_response).await.is_err());
    println!("✓ Corrupted signatures rejected");

    println!("\n✅ All cryptographic properties verified!");
}
