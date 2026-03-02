//! Integration test for P2P messaging between two agents
//!
//! This test verifies that two agents can:
//! 1. Start up and listen on different ports
//! 2. Connect to each other
//! 3. Send messages via gossipsub
//! 4. Receive and process messages

use gork_agent::network::{AgentNetwork, NetworkConfig, NetworkEvent, create_p2p_message};
use gork_agent::types::AgentIdentity;
use std::time::Duration;
use tokio::time::sleep;
use tokio::task::JoinHandle;

#[tokio::test]
async fn test_two_agents_communicate() {
    // Initialize logging
    let _ = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .try_init();

    // Create two agent identities
    let identity1 = AgentIdentity::new("agent1.test".to_string(), vec![1u8; 32]);
    let identity2 = AgentIdentity::new("agent2.test".to_string(), vec![2u8; 32]);

    // Create channels for network events
    let (sender1, mut receiver1) = tokio::sync::mpsc::unbounded_channel();
    let (sender2, mut receiver2) = tokio::sync::mpsc::unbounded_channel();

    // Create network configurations with different ports
    let config1 = NetworkConfig {
        port: 4001,
        bootstrap_peers: vec![],
    };
    let config2 = NetworkConfig {
        port: 4002,
        bootstrap_peers: vec![],
    };

    // Start the first agent
    println!("📦 Starting Agent 1 on port 4001...");
    let mut agent1 = AgentNetwork::new(identity1.clone(), config1, sender1)
        .await
        .expect("Failed to create agent1");

    let addr1 = agent1.listen(Some(4001)).await
        .expect("Failed to listen on port 4001");
    println!("✅ Agent 1 listening on: {}", addr1);
    println!("   Peer ID: {}", agent1.peer_id());

    // Start the second agent
    println!("\n📦 Starting Agent 2 on port 4002...");
    let mut agent2 = AgentNetwork::new(identity2.clone(), config2, sender2)
        .await
        .expect("Failed to create agent2");

    let addr2 = agent2.listen(Some(4002)).await
        .expect("Failed to listen on port 4002");
    println!("✅ Agent 2 listening on: {}", addr2);
    println!("   Peer ID: {}", agent2.peer_id());

    // Give them time to start listening
    sleep(Duration::from_millis(500)).await;

    // Create the multiaddress for agent2 (with peer ID)
    let addr2_with_peer = format!("/ip4/127.0.0.1/tcp/4002/p2p/{}", agent2.peer_id());
    let multiaddr: libp2p::Multiaddr = addr2_with_peer.parse()
        .expect("Failed to parse multiaddr");

    println!("\n🤝 Agent 1 dialing Agent 2 at: {}", multiaddr);

    // Agent 1 dials Agent 2
    if let Err(e) = agent1.dial(multiaddr.clone()).await {
        panic!("Failed to dial: {}", e);
    }

    println!("⏳ Waiting for connection...");

    // Wait a bit for connection to establish
    sleep(Duration::from_secs(2)).await;

    // Check if they connected
    let mut connected = false;
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = &mut timeout => {
                break;
            }
            Some(event) = receiver1.recv() => {
                match event {
                    NetworkEvent::PeerConnected(peer) => {
                        println!("✅ Agent 1 received peer connected: {}", peer);
                        connected = true;
                        break;
                    }
                    NetworkEvent::Error(e) => {
                        println!("❌ Agent 1 error: {}", e);
                    }
                    _ => {}
                }
            }
        }
    }

    if !connected {
        panic!("Agents failed to connect within timeout");
    }

    println!("\n📨 Agent 1 broadcasting message to topic: 'gork-agent-messages'");

    // Agent 1 broadcasts a message
    let test_message = "Hello from Agent 1!";
    let message_bytes = create_p2p_message(test_message);

    if let Err(e) = agent1.broadcast("gork-agent-messages", &message_bytes).await {
        panic!("Failed to broadcast message: {}", e);
    }

    println!("⏳ Waiting for Agent 2 to receive message...");

    // Wait for Agent 2 to receive the message
    let mut message_received = false;
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = &mut timeout => {
                break;
            }
            Some(event) = receiver2.recv() => {
                match event {
                    NetworkEvent::MessageReceived { from, message } => {
                        println!("✅ Agent 2 received message from: {}", from);

                        // Try to parse the message
                        if let Ok(msg_str) = String::from_utf8(message.clone()) {
                            println!("   Raw message: {}", msg_str);
                        }

                        // Verify it's our test message
                        if let Ok(plain_msg) = serde_json::from_slice::<gork_agent::types::PlainMessage>(&message) {
                            println!("   Parsed content: {}", plain_msg.content);
                            if plain_msg.content == test_message {
                                println!("✅ Message content matches!");
                                message_received = true;
                                break;
                            }
                        }
                    }
                    NetworkEvent::PeerConnected(peer) => {
                        println!("✅ Agent 2 received peer connected: {}", peer);
                    }
                    NetworkEvent::Error(e) => {
                        println!("❌ Agent 2 error: {}", e);
                    }
                    _ => {}
                }
            }
        }
    }

    // Give some time for gossipsub propagation
    sleep(Duration::from_secs(1)).await;

    assert!(message_received, "Agent 2 did not receive the message within timeout");

    println!("\n✅ Test PASSED! Two agents successfully communicated via P2P!");
}
