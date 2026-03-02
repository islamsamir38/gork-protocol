use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

mod types;
mod crypto;
mod storage;
mod near;
mod registry;
mod security;
mod network;

use crate::types::AgentIdentity;

/// Default registry contract ID
const DEFAULT_REGISTRY: &str = "registry.gork-agent.testnet";

/// Gork Agent Protocol - P2P agent communication
#[derive(Parser)]
#[command(name = "gork-agent")]
#[command(author = "Gork <irongork.near>")]
#[command(version = "0.1.0")]
#[command(about = "P2P agent-to-agent communication with NEAR integration", long_about = None)]
struct Cli {
    /// Network to use (testnet/mainnet)
    #[arg(short, long, default_value = "testnet")]
    network: String,

    /// Registry contract ID
    #[arg(short, long, default_value = DEFAULT_REGISTRY)]
    registry: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize new agent identity
    Init {
        /// NEAR account ID
        #[arg(short, long)]
        account: String,

        /// Capabilities (comma-separated)
        #[arg(short, long)]
        capabilities: Option<String>,
    },

    /// Show current agent identity
    Whoami,

    /// Show agent status
    Status,

    /// Send message to another agent
    Send {
        /// Recipient account ID
        to: String,

        /// Message content
        message: String,
    },

    /// View inbox messages
    Inbox {
        /// Filter by sender
        #[arg(short, long)]
        from: Option<String>,

        /// Show full message details
        #[arg(short, long)]
        verbose: bool,
    },

    /// Clear inbox
    Clear,

    /// Add capability to agent
    Advertise {
        /// Capability to add
        capability: String,
    },

    /// Discover agents by capability
    Discover {
        /// Capability to search for
        capability: String,

        /// Only show online agents
        #[arg(short, long)]
        online: bool,

        /// Maximum results
        #[arg(short, long, default_value = "10")]
        limit: u32,
    },

    /// List all agents in registry
    List {
        /// Maximum results
        #[arg(short, long, default_value = "20")]
        limit: u64,
    },

    /// Show registry stats
    Stats,

    /// Scan message for security threats
    Scan {
        /// Message content to scan
        message: String,
    },

    /// Show audit log
    Audit {
        /// Number of entries to show
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// List available capabilities
    Capabilities,

    /// Assess risk of a message
    AssessRisk {
        /// Sender account ID
        #[arg(short, long)]
        sender: String,
        
        /// Sender reputation (0-100)
        #[arg(short, long, default_value = "50")]
        reputation: u32,
        
        /// Message content
        message: String,
    },

    /// Start P2P daemon (Phase 3)
    Daemon,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let cli = Cli::parse();
    
    if let Err(e) = run(cli).await {
        eprintln!("❌ Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init { account, capabilities } => {
            init_agent(&account, &cli.network, capabilities)
        }
        Commands::Whoami => whoami(),
        Commands::Status => status(),
        Commands::Send { to, message } => send_message(&to, &message),
        Commands::Inbox { from, verbose } => show_inbox(from, verbose),
        Commands::Clear => clear_inbox(),
        Commands::Advertise { capability } => advertise(&capability),
        Commands::Discover { capability, online, limit } => {
            discover_agents(&cli.registry, &cli.network, &capability, online, limit).await
        }
        Commands::List { limit } => {
            list_agents(&cli.registry, &cli.network, limit).await
        }
        Commands::Stats => {
            show_stats(&cli.registry, &cli.network).await
        }
        Commands::Scan { message } => {
            scan_message(&message)
        }
        Commands::Audit { limit } => {
            show_audit_log(limit)
        }
        Commands::Capabilities => {
            list_capabilities()
        }
        Commands::AssessRisk { sender, reputation, message } => {
            assess_risk(&sender, reputation, &message)
        }
        Commands::Daemon => {
            start_daemon(&cli.registry, &cli.network).await
        }
    }
}

fn get_storage_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".gork-agent")
}

fn init_agent(account: &str, network: &str, capabilities: Option<String>) -> Result<()> {
    let storage_path = get_storage_path();
    std::fs::create_dir_all(&storage_path)?;
    
    let storage = storage::AgentStorage::open(&storage_path)?;
    
    // Check if already initialized
    if let Some(config) = storage.load_config()? {
        println!("⚠️  Agent already initialized: {}", config.identity.account_id);
        println!("   To reinitialize, delete ~/.gork-agent first");
        return Ok(());
    }

    // Create new identity
    let crypto = crypto::MessageCrypto::new()?;
    let public_key = crypto.public_key();
    
    let mut identity = AgentIdentity::new(account.to_string(), public_key);
    
    if let Some(caps) = capabilities {
        let caps: Vec<String> = caps.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        identity.capabilities = caps;
    }

    let config = types::AgentConfig {
        identity: identity.clone(),
        storage_path: storage_path.to_string_lossy().to_string(),
        network_id: network.to_string(),
    };

    storage.save_config(&config)?;
    storage.save_identity(&identity)?;

    println!("✅ Agent initialized successfully!");
    println!("   Account: {}", account);
    println!("   Network: {}", network);
    if !identity.capabilities.is_empty() {
        println!("   Capabilities: {}", identity.capabilities.join(", "));
    }
    println!();
    println!("Next steps:");
    println!("  gork-agent whoami    - View your identity");
    println!("  gork-agent send <to> <message> - Send a message");

    Ok(())
}

fn whoami() -> Result<()> {
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("❌ No agent initialized. Run: gork-agent init --account <your.near>");
        return Ok(());
    }

    let storage = storage::AgentStorage::open(&storage_path)?;
    
    match storage.load_identity()? {
        Some(identity) => {
            println!("🤖 Agent Identity");
            println!("   Account: {}", identity.account_id);
            println!("   Public Key: {}", hex::encode(&identity.public_key).chars().take(16).collect::<String>());
            if !identity.capabilities.is_empty() {
                println!("   Capabilities: {}", identity.capabilities.join(", "));
            }
            if let Some(endpoint) = identity.endpoint {
                println!("   Endpoint: {}", endpoint);
            }
        }
        None => {
            println!("❌ No identity found. Run: gork-agent init --account <your.near>");
        }
    }

    Ok(())
}

fn status() -> Result<()> {
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("❌ No agent initialized");
        return Ok(());
    }

    let storage = storage::AgentStorage::open(&storage_path)?;
    
    match storage.load_config()? {
        Some(config) => {
            let messages = storage.get_messages()?;
            
            println!("📊 Agent Status");
            println!("   Account: {}", config.identity.account_id);
            println!("   Network: {}", config.network_id);
            println!("   Storage: {}", config.storage_path);
            if !config.identity.capabilities.is_empty() {
                println!("   Capabilities: {}", config.identity.capabilities.join(", "));
            }
            println!("   Inbox: {} messages", messages.len());
        }
        None => {
            println!("❌ No configuration found");
        }
    }

    Ok(())
}

fn send_message(to: &str, message: &str) -> Result<()> {
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("❌ No agent initialized. Run: gork-agent init --account <your.near>");
        return Ok(());
    }

    let storage = storage::AgentStorage::open(&storage_path)?;
    
    let config = storage.load_config()?
        .ok_or_else(|| anyhow::anyhow!("No agent configuration found"))?;

    let crypto = crypto::MessageCrypto::new()?;
    
    // Create message
    let plain = types::PlainMessage::new(message.to_string());
    let plaintext = plain.to_bytes();
    
    let payload = types::EncryptedPayload {
        ciphertext: plaintext.clone(),
        nonce: vec![],
        signature: crypto.sign(&plaintext)?,
        sender_pubkey: crypto.public_key(),
    };

    let msg = types::Message::new(
        config.identity.account_id.clone(),
        to.to_string(),
        payload,
    );

    println!("📨 Message prepared");
    println!("   From: {}", msg.from);
    println!("   To: {}", msg.to);
    println!("   ID: {}", msg.id);
    println!();
    println!("   Content: {}", message);
    println!();
    println!("⚠️  Note: P2P delivery not yet implemented (Phase 3)");
    println!("   Message stored locally only");

    // Store locally for now
    storage.save_message(&msg)?;

    Ok(())
}

fn show_inbox(from: Option<String>, verbose: bool) -> Result<()> {
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("📭 Inbox empty (no agent initialized)");
        return Ok(());
    }

    let storage = storage::AgentStorage::open(&storage_path)?;
    
    let messages = match from {
        Some(sender) => storage.get_messages_from(&sender)?,
        None => storage.get_messages()?,
    };

    if messages.is_empty() {
        println!("📭 Inbox empty");
        return Ok(());
    }

    println!("📬 Inbox ({} messages)", messages.len());
    println!();

    for msg in messages {
        let timestamp = chrono::DateTime::from_timestamp_millis(msg.timestamp as i64)
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| msg.timestamp.to_string());

        println!("┌─────────────────────────────────────");
        println!("│ From: {}", msg.from);
        println!("│ Date: {}", timestamp);
        if verbose {
            println!("│ ID: {}", msg.id);
            println!("│ Type: {:?}", msg.message_type);
        }

        // Try to decode content
        if let Ok(plain) = types::PlainMessage::from_bytes(&msg.payload.ciphertext) {
            println!("│");
            println!("│ {}", plain.content);
        } else {
            println!("│ [Encrypted content]");
        }
        println!("└─────────────────────────────────────");
        println!();
    }

    Ok(())
}

fn clear_inbox() -> Result<()> {
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("📭 Nothing to clear");
        return Ok(());
    }

    let storage = storage::AgentStorage::open(&storage_path)?;
    let count = storage.get_messages()?.len();
    
    storage.clear_inbox()?;
    
    println!("🗑️  Cleared {} messages", count);

    Ok(())
}

fn advertise(capability: &str) -> Result<()> {
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("❌ No agent initialized. Run: gork-agent init --account <your.near>");
        return Ok(());
    }

    let mut storage = storage::AgentStorage::open(&storage_path)?;
    let mut config = storage.load_config()?
        .ok_or_else(|| anyhow::anyhow!("No agent configuration found"))?;

    if config.identity.capabilities.contains(&capability.to_string()) {
        println!("ℹ️  Capability already registered: {}", capability);
        return Ok(());
    }

    config.identity.capabilities.push(capability.to_string());
    storage.save_config(&config)?;
    storage.save_identity(&config.identity)?;

    println!("✅ Capability added: {}", capability);
    println!("   Total capabilities: {}", config.identity.capabilities.join(", "));

    Ok(())
}

async fn discover_agents(
    registry_id: &str,
    network: &str,
    capability: &str,
    online_only: bool,
    limit: u32,
) -> Result<()> {
    println!("🔍 Discovering agents with capability: {}", capability);
    println!("   Registry: {}", registry_id);
    println!("   Online only: {}", online_only);
    println!();

    let client = registry::RegistryClient::new(registry_id.to_string(), network);
    
    match client.discover(capability, online_only, limit).await {
        Ok(agents) => {
            if agents.is_empty() {
                println!("📭 No agents found with capability: {}", capability);
            } else {
                println!("📋 Found {} agent(s):", agents.len());
                println!();
                
                for agent in agents {
                    let status = if agent.online { "🟢" } else { "🔴" };
                    println!("{} {} ({})", status, agent.name, agent.account_id);
                    println!("   Reputation: {} ({})", agent.reputation, agent.rating_count);
                    if let Some(ref endpoint) = agent.endpoint {
                        println!("   Endpoint: {}", endpoint);
                    }
                    println!("   Capabilities: {}", agent.capabilities.join(", "));
                    if !agent.description.is_empty() {
                        println!("   Description: {}", agent.description);
                    }
                    println!();
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to query registry: {}", e);
            println!("   Make sure the registry contract is deployed");
        }
    }

    Ok(())
}

async fn list_agents(registry_id: &str, network: &str, limit: u64) -> Result<()> {
    println!("📋 Listing all agents (limit: {})", limit);
    println!("   Registry: {}", registry_id);
    println!();

    let client = registry::RegistryClient::new(registry_id.to_string(), network);
    
    match client.get_all_agents(0, limit).await {
        Ok(agents) => {
            if agents.is_empty() {
                println!("📭 No agents registered");
            } else {
                println!("📋 {} agent(s) registered:", agents.len());
                println!();
                
                for agent in agents {
                    let status = if agent.online { "🟢" } else { "🔴" };
                    println!("{} {} - {}", status, agent.account_id, agent.name);
                    println!("   Capabilities: {}", agent.capabilities.join(", "));
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to query registry: {}", e);
        }
    }

    Ok(())
}

async fn show_stats(registry_id: &str, network: &str) -> Result<()> {
    println!("📊 Registry Statistics");
    println!("   Contract: {}", registry_id);
    println!("   Network: {}", network);
    println!();

    let client = registry::RegistryClient::new(registry_id.to_string(), network);
    
    let total = client.get_total_count().await.unwrap_or(0);
    let online = client.get_online_count().await.unwrap_or(0);

    println!("   Total agents: {}", total);
    println!("   Online now: {} ({:.0}%)", online, if total > 0 { (online as f64 / total as f64) * 100.0 } else { 0.0 });

    Ok(())
}

fn scan_message(message: &str) -> Result<()> {
    println!("🔍 Scanning message for security threats...");
    println!();
    
    let filter = security::ContentFilter::new();
    
    match filter.scan(message) {
        security::ScanResult::Safe => {
            println!("✅ Message is safe");
        }
        security::ScanResult::Warning { reason, .. } => {
            println!("⚠️  Warning detected:");
            println!("   {}", reason);
            println!();
            println!("   Message: {}", message);
        }
        security::ScanResult::Blocked { reason } => {
            println!("🚫 Message blocked:");
            println!("   {}", reason);
            println!();
            println!("   Message: {}", message);
        }
    }
    
    // Also validate input
    match security::InputValidator::validate(message.as_bytes()) {
        Ok(validated) => {
            println!();
            println!("📏 Size: {} bytes", message.len());
            if validated.json.is_some() {
                println!("📦 Format: Valid JSON");
            } else {
                println!("📦 Format: Plain text");
            }
        }
        Err(e) => {
            println!();
            println!("❌ Validation error: {}", e);
        }
    }
    
    Ok(())
}

fn show_audit_log(limit: usize) -> Result<()> {
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("📭 No audit log (agent not initialized)");
        return Ok(());
    }
    
    // For now, show a placeholder - in full implementation would load from storage
    println!("📋 Audit Log (last {} entries)", limit);
    println!();
    println!("   Note: Audit log persisted in memory only for Phase 2");
    println!("   Will be stored in RocksDB in Phase 3");
    println!();
    println!("   Use 'gork-agent scan <message>' to check messages");
    
    Ok(())
}

fn list_capabilities() -> Result<()> {
    println!("🔧 Available Capabilities");
    println!();
    
    let scope = security::CapabilityScope::new();
    
    for cap in scope.list_capabilities() {
        let policy = match &cap.allowed_callers {
            security::CallerPolicy::Anyone => "Anyone",
            security::CallerPolicy::RegisteredAgents => "Registered",
            security::CallerPolicy::TrustedOnly => "Trusted (rep≥50)",
            security::CallerPolicy::Whitelist(_) => "Whitelist",
            security::CallerPolicy::OwnerOnly => "Owner Only",
        };
        
        let risk = match cap.risk_level {
            security::RiskLevel::Low => "🟢 Low",
            security::RiskLevel::Medium => "🟡 Medium",
            security::RiskLevel::High => "🟠 High",
            security::RiskLevel::Critical => "🔴 Critical",
        };
        
        let approval = if cap.requires_approval { "✓" } else { "—" };
        
        println!("📦 {}", cap.name);
        println!("   {}", cap.description);
        println!("   Policy: {} | Risk: {} | Approval: {}", policy, risk, approval);
        println!();
    }
    
    Ok(())
}

fn assess_risk(sender: &str, reputation: u32, message: &str) -> Result<()> {
    println!("🎯 Risk Assessment");
    println!();
    println!("   Sender: {}", sender);
    println!("   Reputation: {}", reputation);
    println!("   Message: {}", message);
    println!();
    
    let analyzer = security::RiskAnalyzer::new();
    let is_known = reputation >= 50;
    let assessment = analyzer.assess(sender, message, reputation, is_known);
    
    let level = match assessment.level {
        security::RiskLevel::Low => "🟢 Low",
        security::RiskLevel::Medium => "🟡 Medium",
        security::RiskLevel::High => "🟠 High",
        security::RiskLevel::Critical => "🔴 Critical",
    };
    
    let recommendation = match assessment.recommendation {
        security::Recommendation::Allow => "✅ Allow",
        security::Recommendation::RequireApproval => "⚠️  Require Approval",
        security::Recommendation::Deny => "🚫 Deny",
        security::Recommendation::Escalate => "🚨 Escalate to Human",
    };
    
    println!("📊 Results:");
    println!("   Score: {}/100", assessment.score);
    println!("   Level: {}", level);
    println!("   Recommendation: {}", recommendation);
    
    if !assessment.factors.is_empty() {
        println!();
        println!("🔍 Risk Factors:");
        for factor in &assessment.factors {
            println!("   • {}", factor);
        }
    }
    
    Ok(())
}

async fn start_daemon(_registry_id: &str, _network: &str) -> Result<()> {
    println!("🚀 Starting Gork Agent P2P Daemon");
    println!();
    
    // Load agent identity
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("❌ No agent initialized. Run: gork-agent init --account <your.near>");
        return Ok(());
    }
    
    let storage = storage::AgentStorage::open(&storage_path)?;
    let config = storage.load_config()?
        .ok_or_else(|| anyhow::anyhow!("No agent configuration found"))?;
    
    println!("🤖 Agent: {}", config.identity.account_id);
    println!();
    
    // Create event channel
    let (event_sender, mut event_receiver) = tokio::sync::mpsc::unbounded_channel();
    
    println!("🌐 Initializing P2P network...");
    
    // Create network
    let mut p2p_network = network::AgentNetwork::new(
        config.identity.clone(),
        network::NetworkConfig::default(),
        event_sender,
    ).await?;
    
    // Start listening
    let listen_addr = p2p_network.listen(None)?;
    println!("📡 Listening on: {}", listen_addr);
    println!("   Peer ID: {}", p2p_network.peer_id());
    println!();
    
    println!("✅ Daemon started successfully!");
    println!("   Press Ctrl+C to stop");
    println!();
    
    // Create message handler
    let mut message_handler = network::MessageHandler::new(&config.identity.account_id);
    
    // Run network event loop
    loop {
        tokio::select! {
            Some(event) = event_receiver.recv() => {
                match event {
                    network::NetworkEvent::MessageReceived { from, topic, message } => {
                        println!("📨 Message from {} on '{}'", from, topic);
                        
                        if let Ok(Some(msg)) = message_handler.handle_message(from, &message) {
                            if let Ok(plain) = types::PlainMessage::from_bytes(&msg.payload.ciphertext) {
                                println!("   Content: {}", plain.content);
                            }
                        }
                    }
                    network::NetworkEvent::PeerConnected(peer) => {
                        println!("🟢 Peer connected: {}", peer);
                    }
                    network::NetworkEvent::PeerDisconnected(peer) => {
                        println!("🔴 Peer disconnected: {}", peer);
                    }
                    network::NetworkEvent::Error(e) => {
                        println!("❌ Error: {}", e);
                    }
                }
            }
            
            _ = tokio::signal::ctrl_c() => {
                println!("\n🛑 Shutting down...");
                break;
            }
        }
    }
    
    println!("👋 Daemon stopped");
    Ok(())
}
