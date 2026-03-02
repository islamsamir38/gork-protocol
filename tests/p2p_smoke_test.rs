//! Smoke test for P2P functionality
//!
//! This test verifies that:
//! 1. Agent networks can be created
//! 2. They can listen on ports
//! 3. The build fixes work correctly

use gork_agent::network::{AgentNetwork, NetworkConfig};
use gork_agent::types::AgentIdentity;

#[tokio::test]
async fn test_create_agent_network() {
    println!("🧪 Testing P2P network creation...");

    // Create an agent identity
    let identity = AgentIdentity::new("test.test".to_string(), vec![1u8; 32]);

    // Create event channel
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();

    // Create network configuration
    let config = NetworkConfig {
        port: 0, // Use random available port
        bootstrap_peers: vec![],
    };

    // Create the network
    println!("📦 Creating agent network...");
    let result = AgentNetwork::new(identity, config, sender).await;

    assert!(result.is_ok(), "Failed to create agent network: {:?}", result.err());

    let network = result.unwrap();
    println!("✅ Agent network created successfully!");
    println!("   Peer ID: {}", network.peer_id());

    // Test listening
    println!("\n🔌 Testing listen on random port...");
    let listen_result = network.listen(Some(0)).await;
    assert!(listen_result.is_ok(), "Failed to listen: {:?}", listen_result.err());

    let addr = listen_result.unwrap();
    println!("✅ Listening on: {}", addr);

    println!("\n✅ Test PASSED! P2P network creation works correctly.");
}
