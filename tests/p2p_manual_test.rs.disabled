//! Manual P2P messaging test
//!
//! This test creates two agents and verifies they can connect and communicate

use gork_agent::network::NetworkConfig;
use gork_agent::types::AgentIdentity;
use libp2p::{
    gossipsub, identify, kad, noise, ping, swarm::NetworkBehaviour, swarm::SwarmEvent, tcp, yamux,
    Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use libp2p::futures::StreamExt;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::time::sleep;

#[derive(NetworkBehaviour)]
struct TestBehaviour {
    gossipsub: gossipsub::Behaviour,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

#[tokio::test]
async fn test_manual_p2p_connection() {
    // Initialize logging
    let _ = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .try_init();

    println!("🧪 Testing P2P connection between two agents...");

    // Create Agent 1
    println!("\n📦 Creating Agent 1...");
    let swarm1 = create_test_swarm().await;
    let peer_id1 = *swarm1.local_peer_id();
    println!("✅ Agent 1 peer ID: {}", peer_id1);

    // Create Agent 2
    println!("📦 Creating Agent 2...");
    let swarm2 = create_test_swarm().await;
    let peer_id2 = *swarm2.local_peer_id();
    println!("✅ Agent 2 peer ID: {}", peer_id2);

    // Make swarm1 listen
    println!("\n🔌 Making Agent 1 listen on port 4001...");
    let mut swarm1 = swarm1;
    swarm1.listen_on("/ip4/0.0.0.0/tcp/4001".parse().unwrap()).unwrap();

    // Make swarm2 listen
    println!("🔌 Making Agent 2 listen on port 4002...");
    let mut swarm2 = swarm2;
    swarm2.listen_on("/ip4/0.0.0.0/tcp/4002".parse().unwrap()).unwrap();

    // Wait for them to start listening
    sleep(Duration::from_millis(500)).await;

    // Get agent2's address with peer ID
    let addr2 = format!("/ip4/127.0.0.1/tcp/4002/p2p/{}", peer_id2);
    let multiaddr: Multiaddr = addr2.parse().unwrap();

    println!("\n🤝 Agent 1 dialing Agent 2: {}", multiaddr);
    swarm1.dial(multiaddr).unwrap();

    println!("⏳ Waiting for connection...");

    // Poll both swarms and check for connection
    let mut connected = false;
    let mut attempts = 0;
    let max_attempts = 20;

    while !connected && attempts < max_attempts {
        // Poll swarm1
        match swarm1.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("   Agent1 listening on: {}", address);
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                println!("✅ Agent1 connected to: {}", peer_id);
                if peer_id == peer_id2 {
                    connected = true;
                }
            }
            SwarmEvent::OutgoingConnectionError { error, .. } => {
                println!("❌ Agent1 connection error: {}", error);
            }
            _ => {}
        }

        if !connected {
            // Also poll swarm2 to process incoming connection
            match swarm2.try_select_next_some() {
                Ok(Some(event)) => {
                    if let SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
                        println!("✅ Agent2 connected to: {}", peer_id);
                        if peer_id == peer_id1 {
                            connected = true;
                        }
                    }
                }
                _ => {}
            }
        }

        attempts += 1;
        sleep(Duration::from_millis(100)).await;
    }

    if !connected {
        panic!("Agents failed to connect within timeout");
    }

    println!("\n✅ Test PASSED! Agents successfully connected via P2P!");
}

async fn create_test_swarm() -> Swarm<TestBehaviour> {
    // Create gossipsub config
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .message_id_fn(|message: &gossipsub::Message| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            gossipsub::MessageId::from(s.finish().to_string())
        })
        .build()
        .unwrap();

    // Create gossipsub behaviour
    let mut gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(libp2p::identity::Keypair::generate_ed25519()),
        gossipsub_config,
    ).unwrap();

    // Subscribe to topic
    let topic = gossipsub::IdentTopic::new("gork-agent-messages");
    gossipsub.subscribe(&topic).unwrap();

    // Create Kademlia DHT
    let temp_peer_id = PeerId::random();
    let store = kad::store::MemoryStore::new(temp_peer_id);
    let mut kademlia = kad::Behaviour::new(temp_peer_id, store);
    kademlia.set_mode(Some(kad::Mode::Server));

    // Create identify behaviour
    let identify_keypair = libp2p::identity::Keypair::generate_ed25519();
    let identify = identify::Behaviour::new(identify::Config::new(
        "/gork-agent/1.0.0".to_string(),
        identify_keypair.public(),
    ));

    // Create ping behaviour
    let ping = ping::Behaviour::new(ping::Config::new());

    // Create behaviour
    let behaviour = TestBehaviour {
        gossipsub,
        kademlia,
        identify,
        ping,
    };

    // Create swarm
    SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .unwrap()
        .with_behaviour(|_| behaviour)
        .unwrap()
        .with_swarm_config(|config| {
            config.with_idle_connection_timeout(Duration::from_secs(60))
        })
        .build()
}
