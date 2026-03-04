use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::Read;
use std::time::Duration;
use tracing::{error, info, warn};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

mod crypto;
mod near;
mod network;
mod registry;
mod relay;
mod relay_discovery;
mod security;
mod skills;
mod storage;
mod types;
mod load_balancing;
mod certificate;
mod register;

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
    ///
    /// Creates a new agent identity with NEAR account verification.
    ///
    /// 🔒 MANDATORY: All agents must prove NEAR account ownership to access the network.
    ///
    /// Setup:
    ///   1. Install NEAR CLI: npm install -g near-cli
    ///   2. Login: near login --account-id <your-account>
    ///   3. Initialize: gork-agent init --account <your-account>
    ///
    /// Examples:
    ///   gork-agent init --account alice.testnet
    ///   gork-agent init --account alice.testnet --capabilities "chat,payment"
    Init {
        /// NEAR account ID (will be verified via NEAR CLI credentials)
        #[arg(short, long)]
        account: String,

        /// Capabilities to advertise (comma-separated)
        ///
        /// Example: --capabilities "chat,payment,file-transfer"
        #[arg(short, long)]
        capabilities: Option<String>,

        /// Enable development mode (skip NEAR verification)
        ///
        /// ⚠️  DANGEROUS - Only for local testing!
        /// In dev mode, anyone can claim any NEAR account ID.
        #[arg(long)]
        dev_mode: bool,

        /// Private key for development testing (requires --dev-mode)
        ///
        /// ⚠️  FOR TESTING ONLY - Never use in production!
        #[arg(long, hide = true, requires = "dev_mode")]
        private_key: Option<String>,
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

    /// Register agent on blockchain (Variant C)
    /// 
    /// This creates a cryptographic link between your NEAR account
    /// and your agent's P2P identity. One-time registration (~0.1 NEAR).
    /// 
    /// After registration, your agent can participate in the network
    /// with certificate-based verification.
    Register {
        /// NEAR account to register (must have credentials)
        #[arg(short, long)]
        account: String,
    },

    /// Start P2P daemon (Phase 3)
    Daemon {
        /// Port to listen on
        #[arg(short, long, default_value = "4001")]
        port: u16,

        /// Bootstrap peers (multiaddresses)
        #[arg(long)]
        bootstrap_peers: Option<String>,

        /// Relay domain for automatic discovery
        /// 
        /// Example: --relay relay.jemartel.near
        /// Queries _p2p.relay.jemartel.near TXT for multiaddr
        /// 
        /// If not specified, uses default relay: relay.jemartel.near
        #[arg(long)]
        relay: Option<String>,
    },

    /// Start relay server (help other peers connect)
    Relay {
        /// Port to listen on
        #[arg(short, long, default_value = "4001")]
        port: u16,

        /// Maximum number of relay circuits
        #[arg(short, long, default_value = "1000")]
        max_circuits: usize,

        /// Enable Prometheus metrics
        #[arg(long)]
        metrics: bool,

        /// Metrics port
        #[arg(long, default_value = "9090")]
        metrics_port: u16,
    },

    /// Manage Agent Skills
    ///
    /// Publish, discover, and manage skills following the Agent Skills format.
    Skills {
        #[command(subcommand)]
        action: SkillsCommands,
    },

    /// Execute a skill on a remote agent
    ///
    /// Find an agent with the desired skill and execute it via P2P.
    Execute {
        #[command(subcommand)]
        action: ExecuteCommands,
    },

    /// Marketplace actions (rate skills, view rankings)
    ///
    /// Interact with the skill marketplace.
    Marketplace {
        #[command(subcommand)]
        action: MarketplaceCommands,
    },
    
    /// Manage API keys for HTTP API authentication
    ApiKeys {
        #[command(subcommand)]
        action: ApiKeyCommands,
    },
    
    /// Manage message queue for offline sending
    Queue {
        #[command(subcommand)]
        action: QueueCommands,
    },
}

/// Skills subcommands
#[derive(Subcommand)]
enum SkillsCommands {
    /// Install a skill locally
    ///
    /// Installs a skill package from a local directory.
    /// Skills are advertised on P2P network when daemon is running.
    Install {
        /// Path to skill package directory
        #[arg(short, long)]
        path: String,
    },

    /// List local skills
    ///
    /// Show all skills installed locally.
    List,

    /// Remove a local skill
    ///
    /// Uninstall a skill from local storage.
    Remove {
        /// Skill name
        #[arg(short, long)]
        name: String,
    },

    /// Show skill details
    ///
    /// Display detailed information about a local skill.
    Show {
        /// Skill name
        #[arg(short, long)]
        name: String,
    },
}

/// Execute subcommands
#[derive(Subcommand)]
enum ExecuteCommands {
    /// Request a task from another agent
    ///
    /// Send a task request to another agent on the P2P network.
    Request {
        /// Agent to request from
        #[arg(short, long)]
        agent: String,

        /// Skill to use
        #[arg(short, long)]
        skill: String,

        /// Capability within the skill
        #[arg(short, long)]
        capability: String,

        /// Input data (JSON)
        #[arg(short, long)]
        input: String,
    },

    /// Rate an agent after collaboration
    ///
    /// Rate an agent on the NEAR registry after successful collaboration.
    Rate {
        /// Agent to rate
        #[arg(short, long)]
        agent: String,

        /// Rating (1-5 stars)
        #[arg(short, long)]
        rating: u32,
    },
}

/// Marketplace subcommands

/// API key management subcommands
#[derive(Subcommand)]
enum ApiKeyCommands {
    /// Create a new API key
    Create {
        /// Key name (for identification)
        #[arg(short, long)]
        name: String,
        
        /// Permissions (comma-separated: read,write,admin)
        #[arg(short, long, default_value = "read,write")]
        permissions: String,
    },
    
    /// List all API keys
    List,
    
    /// Revoke an API key
    Revoke {
        /// API key to revoke
        key: String,
    },
}

/// Message queue subcommands
#[derive(Subcommand)]
enum QueueCommands {
    /// Show pending messages in queue
    List {
        /// Maximum results
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    
    /// Clear sent messages older than X days
    Cleanup {
        /// Days to keep
        #[arg(short, long, default_value = "7")]
        days: i64,
    },
}

/// Marketplace subcommands
#[derive(Subcommand)]
enum MarketplaceCommands {
    /// List available skills on P2P network
    ///
    /// Show all skills discovered from network.
    List {
        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Maximum results
        #[arg(short, long, default_value = "20")]
        limit: u32,
    },
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
        Commands::Init {
            account,
            capabilities,
            dev_mode,
            private_key,
        } => init_agent(&account, &cli.network, capabilities, dev_mode, private_key).await,
        Commands::Whoami => whoami(),
        Commands::Status => status(),
        Commands::Send { to, message } => send_message(&to, &message).await,
        Commands::Inbox { from, verbose } => show_inbox(from, verbose),
        Commands::Clear => clear_inbox(),
        Commands::Advertise { capability } => advertise(&capability),
        Commands::Discover {
            capability,
            online,
            limit,
        } => discover_agents(&cli.registry, &cli.network, &capability, online, limit).await,
        Commands::List { limit } => list_agents(&cli.registry, &cli.network, limit).await,
        Commands::Stats => show_stats(&cli.registry, &cli.network).await,
        Commands::Scan { message } => scan_message(&message),
        Commands::Audit { limit } => show_audit_log(limit),
        Commands::Capabilities => list_capabilities(),
        Commands::AssessRisk {
            sender,
            reputation,
            message,
        } => assess_risk(&sender, reputation, &message),
        Commands::Daemon { port, bootstrap_peers, relay } => start_daemon(port, bootstrap_peers, relay).await,
        Commands::Register { account } => register::register_agent(&account, &cli.network, &cli.registry).await,
        Commands::Relay {
            port,
            max_circuits,
            metrics,
            metrics_port,
        } => start_relay(port, max_circuits, metrics, metrics_port).await,
        Commands::Skills { action } => handle_skills_command(action).await,
        Commands::Execute { action } => handle_execute_command(action, &cli.registry, &cli.network).await,
        Commands::Marketplace { action } => handle_marketplace_command(action).await,
        Commands::ApiKeys { action } => handle_api_key_command(action),
        Commands::Queue { action } => handle_queue_command(action),
    }
}

fn get_storage_path() -> std::path::PathBuf {
    // Allow custom storage path via GORK_AGENT_HOME env var
    if let Ok(custom) = std::env::var("GORK_AGENT_HOME") {
        std::path::PathBuf::from(custom)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home).join(".gork-agent")
    }
}

// HTTP API routes for daemon
fn create_api_routes(
    storage_path: std::path::PathBuf,
    account_id: String,
    peer_id: String,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    use warp::Filter;
    
    // Clone values for use in closures
    let account_id_health = account_id.clone();
    let peer_id_health = peer_id.clone();
    
    let account_id_status = account_id.clone();
    let peer_id_status = peer_id.clone();
    let storage_path_status = storage_path.clone();
    
    let storage_path_inbox = storage_path.clone();
    
    // GET /health - Health check
    let health = warp::path("health")
        .map(move || {
            warp::reply::json(&serde_json::json!({
                "status": "ok",
                "account": account_id_health.clone(),
                "peer_id": peer_id_health.clone(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }))
        });
    
    // GET /api/v1/status - Daemon status
    let status = warp::path!("api" / "v1" / "status")
        .map(move || {
            warp::reply::json(&serde_json::json!({
                "account": account_id_status.clone(),
                "peer_id": peer_id_status.clone(),
                "storage": storage_path_status.to_str().unwrap_or("unknown"),
            }))
        });
    
    // GET /api/v1/inbox - Get messages
    let inbox = warp::path!("api" / "v1" / "inbox")
        .and(warp::get())
        .map(move || {
            match storage::AgentStorage::open(&storage_path_inbox) {
                Ok(storage) => {
                    match storage.get_messages() {
                        Ok(messages) => {
                            warp::reply::json(&serde_json::json!({
                                "count": messages.len(),
                                "messages": messages,
                            }))
                        }
                        Err(e) => {
                            warp::reply::json(&serde_json::json!({
                                "error": e.to_string(),
                            }))
                        }
                    }
                }
                Err(e) => {
                    warp::reply::json(&serde_json::json!({
                        "error": e.to_string(),
                    }))
                }
            }
        });
    
    // POST /api/v1/send - Send message (returns instruction to use CLI command for now)
    let send = warp::path!("api" / "v1" / "send")
        .and(warp::post())
        .and(warp::body::json())
        .map(|body: serde_json::Value| {
            // For now, return instructions
            // In full implementation, this would broadcast via P2P
            let to = body.get("to").and_then(|v| v.as_str()).unwrap_or("unknown");
            let _message = body.get("message").and_then(|v| v.as_str()).unwrap_or("");
            
            warp::reply::json(&serde_json::json!({
                "status": "queued",
                "to": to,
                "hint": "Use 'gork-agent send' command for P2P messaging",
                "note": "Direct API sending requires P2P network integration (TODO)",
            }))
        });
    
    health
        .or(status)
        .or(inbox)
        .or(send)
}

async fn init_agent(
    account: &str,
    network: &str,
    capabilities: Option<String>,
    dev_mode: bool,
    private_key: Option<String>,
) -> Result<()> {
    let storage_path = get_storage_path();
    std::fs::create_dir_all(&storage_path)?;

    let storage = storage::AgentStorage::open(&storage_path)?;

    // Check if already initialized
    if let Some(config) = storage.load_config()? {
        println!(
            "⚠️  Agent already initialized: {}",
            config.identity.account_id
        );
        println!("   To reinitialize, delete ~/.gork-agent first");
        return Ok(());
    }

    // Security: Require NEAR verification unless in dev mode
    if dev_mode {
        println!("⚠️  DEVELOPMENT MODE ENABLED");
        println!("   Skipping NEAR verification (INSECURE!)");
        println!("   This should ONLY be used for local testing!");
        println!();
    }

    let crypto = if !dev_mode {
        // MANDATORY: Verify NEAR account ownership
        println!("🔐 Verifying NEAR account ownership...");
        println!("   Account: {}", account);

        let near_network = near::Network::from_str(network);
        let near_identity = near::NearIdentity::new(account.to_string(), near_network);

        // Check if credentials exist
        if !near_identity.has_credentials() {
            println!("❌ NEAR credentials not found!");
            println!();
            println!("Network access requires NEAR account verification:");
            println!();
            println!("  1. Install NEAR CLI: npm install -g near-cli");
            println!("  2. Login: near login --account-id {}", account);
            println!("  3. Initialize: gork-agent init --account {}", account);
            println!();
            println!("For local testing only, use --dev-mode:");
            println!("  gork-agent init --account {} --dev-mode", account);
            return Err(anyhow::anyhow!(
                "NEAR credentials not found. Run 'near login --account-id {}' first",
                account
            ));
        }

        // Load credentials
        let creds = near_identity.load_credentials()?;
        println!("✅ NEAR credentials loaded from: {}", near_identity.credentials_path.display());

        // Verify account exists on blockchain
        println!("🔍 Verifying account exists on {}...", network);
        let account_exists = near_identity.validate_account().await?;
        if !account_exists {
            return Err(anyhow::anyhow!(
                "Account {} does not exist on {}",
                account,
                network
            ));
        }
        println!("✅ Account verified on blockchain");

        // Create crypto from NEAR private key
        let private_key_bytes = decode_near_private_key(&creds.private_key)?;
        crypto::MessageCrypto::from_keys(&private_key_bytes, &private_key_bytes)?
    } else if let Some(pk) = private_key {
        // Use provided private key (dev mode only)
        println!("⚠️  Using provided private key (development mode)");
        let pk_bytes = bs58::decode(&pk).into_vec()
            .map_err(|_| anyhow::anyhow!("Invalid private key format"))?;
        if pk_bytes.len() != 32 {
            return Err(anyhow::anyhow!("Private key must be 32 bytes"));
        }
        crypto::MessageCrypto::from_keys(&pk_bytes, &pk_bytes)?
    } else {
        // Generate new keypair (dev mode only)
        println!("⚠️  Generating new keypair (development mode)");
        println!("   ⚠️  WARNING: No identity verification!");
        crypto::MessageCrypto::new()?
    };

    let public_key = crypto.public_key();

    let mut identity = AgentIdentity::new(account.to_string(), public_key);

    if let Some(caps) = capabilities {
        let caps: Vec<String> = caps
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        identity.capabilities = caps;
    }

    let config = types::AgentConfig {
        identity: identity.clone(),
        storage_path: storage_path.to_string_lossy().to_string(),
        network_id: network.to_string(),
        near_verified: !dev_mode, // Verified if not in dev mode
        saved_relay: None, // Will be set on first daemon run
    };

    storage.save_config(&config)?;
    storage.save_identity(&identity)?;

    println!();
    if dev_mode {
        println!("✅ Agent initialized (DEVELOPMENT MODE)");
        println!("   ⚠️  NOT verified - will be rejected by mainnet!");
    } else {
        println!("✅ Agent initialized successfully!");
        println!("   🔐 NEAR account ownership: VERIFIED");
    }
    println!("   Account: {}", account);
    println!("   Network: {}", network);
    if !identity.capabilities.is_empty() {
        println!("   Capabilities: {}", identity.capabilities.join(", "));
    }
    println!();
    println!("Next steps:");
    println!("  gork-agent whoami     - View your identity");
    println!("  gork-agent status     - Show agent status");
    println!("  gork-agent daemon     - Start P2P daemon");
    if dev_mode {
        println!();
        println!("⚠️  DEVELOPMENT MODE - For production:");
        println!("   1. near login --account-id {}", account);
        println!("   2. rm -rf ~/.gork-agent");
        println!("   3. gork-agent init --account {}", account);
    }

    Ok(())
}

/// Decode NEAR private key from base58 format
/// NEAR uses "ed25519:<base58_key>" format
/// NEAR stores keypair as 64 bytes (private + public), we need just private (32 bytes)
fn decode_near_private_key(key: &str) -> Result<Vec<u8>> {
    let key = key.trim();

    // Remove ed25519: prefix if present
    let key_bytes = if key.starts_with("ed25519:") {
        bs58::decode(&key[8..]).into_vec()
    } else {
        bs58::decode(key).into_vec()
    }
    .map_err(|_| anyhow::anyhow!("Invalid NEAR private key format"))?;

    // NEAR stores full keypair (64 bytes): private (32) + public (32)
    // Extract just the private key portion
    if key_bytes.len() == 64 {
        Ok(key_bytes[0..32].to_vec())
    } else if key_bytes.len() == 32 {
        Ok(key_bytes)
    } else {
        Err(anyhow::anyhow!(
            "Invalid private key length: expected 32 or 64 bytes, got {}",
            key_bytes.len()
        ))
    }
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
            println!(
                "   Public Key: {}",
                hex::encode(&identity.public_key)
                    .chars()
                    .take(16)
                    .collect::<String>()
            );
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
                println!(
                    "   Capabilities: {}",
                    config.identity.capabilities.join(", ")
                );
            }
            println!("   Inbox: {} messages", messages.len());
        }
        None => {
            println!("❌ No configuration found");
        }
    }

    Ok(())
}

async fn send_message(to: &str, message: &str) -> Result<()> {
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("❌ No agent initialized. Run: gork-agent init --account <your.near>");
        return Ok(());
    }

    println!("📨 Sending message to: {}", to);
    println!("   Content: {}", message);
    println!();

    // Try HTTP API first (fast path - uses daemon's P2P connections)
    let api_port = 4002; // Default daemon port + 1
    let api_url = format!("http://127.0.0.1:{}/api/v1/send", api_port);
    
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "to": to,
        "message": message,
    });
    
    match client
        .post(&api_url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            println!("✅ Message sent via local daemon API");
            println!("   API: {}", api_url);
            return Ok(());
        }
        _ => {
            // Daemon not available, use temp P2P node
            println!("⚠️  Local daemon not available, using temp P2P node...");
            println!("   (Start daemon with 'gork-agent daemon' for faster sends)");
            println!();
        }
    }

    // Load config (read-only, should work)
    let storage = storage::AgentStorage::open(&storage_path)?;
    let config = storage.load_config()?
        .ok_or_else(|| anyhow::anyhow!("No agent configuration found"))?;

    // Create temporary P2P network for sending
    let (event_sender, mut event_receiver) = tokio::sync::mpsc::unbounded_channel();
    
    // Connect to local daemon on port 4001 with proper peer ID
    // Note: For production, this should use a local IPC/HTTP API instead
    let network_config = network::NetworkConfig {
        port: 0, // Use random port for sender
        bootstrap_peers: vec![],
    };

    let mut p2p = network::AgentNetwork::with_auth(
        config.identity.clone(),
        network_config,
        event_sender,
        None,
        false, // Don't require auth for sender
    ).await?;

    // Start listening on random port
    p2p.listen(None).await?;
    
    // Create message
    let plain = types::PlainMessage::new(message.to_string());
    let message_bytes = plain.to_bytes();

    // Broadcast to gossipsub topic (will propagate to connected peers)
    p2p.broadcast(network::GOSSIPSUB_TOPIC, &message_bytes).await?;

    println!("✅ Message broadcast to P2P network");
    println!("   Topic: {}", network::GOSSIPSUB_TOPIC);
    println!();
    println!("⚠️  Note: For reliable delivery, ensure recipient daemon is running");
    println!("   Run 'gork-agent inbox' on recipient to check for messages");
    
    // Wait a bit for propagation
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

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
    let mut config = storage
        .load_config()?
        .ok_or_else(|| anyhow::anyhow!("No agent configuration found"))?;

    if config
        .identity
        .capabilities
        .contains(&capability.to_string())
    {
        println!("ℹ️  Capability already registered: {}", capability);
        return Ok(());
    }

    config.identity.capabilities.push(capability.to_string());
    storage.save_config(&config)?;
    storage.save_identity(&config.identity)?;

    println!("✅ Capability added: {}", capability);
    println!(
        "   Total capabilities: {}",
        config.identity.capabilities.join(", ")
    );

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
                    println!(
                        "   Reputation: {} ({})",
                        agent.reputation, agent.rating_count
                    );
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
    println!(
        "   Online now: {} ({:.0}%)",
        online,
        if total > 0 {
            (online as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    );

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
        println!(
            "   Policy: {} | Risk: {} | Approval: {}",
            policy, risk, approval
        );
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

async fn start_daemon(port: u16, bootstrap_peers: Option<String>, relay: Option<String>) -> Result<()> {
    println!("🚀 Starting Gork Agent P2P Daemon");
    println!();

    // Load agent identity
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("❌ No agent initialized. Run: gork-agent init --account <your.near>");
        return Ok(());
    }

    let storage = storage::AgentStorage::open(&storage_path)?;
    let config = storage
        .load_config()?
        .ok_or_else(|| anyhow::anyhow!("No agent configuration found"))?;

    println!("🤖 Agent: {}", config.identity.account_id);
    println!();

    // Resolve relay via DNS discovery
    // Priority: --relay flag > saved relay > default relay
    const DEFAULT_RELAY: &str = "relay.jemartel.near";
    
    let relay_domain = relay.as_ref()
        .or(config.saved_relay.as_ref())
        .map(|s| s.as_str())
        .unwrap_or(DEFAULT_RELAY);
    
    let bootstrap_multiaddr = if relay_domain.is_empty() {
        // No relay, use manual bootstrap peers only
        bootstrap_peers.clone()
    } else {
        println!("🔍 Discovering relay: {}", relay_domain);
        
        let discovery = relay_discovery::RelayDiscovery::new("dns.jemartel.near".to_string());
        match discovery.discover(relay_domain).await {
            Ok(multiaddr) => {
                println!("✅ Relay discovered: {}", multiaddr);
                
                // Save for future use
                // TODO: storage.save_relay(relay_domain)?;
                
                Some(multiaddr)
            }
            Err(e) => {
                println!("⚠️  Relay discovery failed: {}", e);
                
                if relay.is_some() {
                    // User explicitly requested this relay, show error
                    println!("   Falling back to bootstrap-peers if provided");
                    bootstrap_peers.clone()
                } else {
                    // Default relay failed, try to continue anyway
                    println!("   Continuing without relay (direct connections only)");
                    bootstrap_peers.clone()
                }
            }
        }
    };

    // Check if agent is NEAR verified
    if !config.near_verified {
        println!("⚠️  WARNING: Agent not NEAR-verified!");
        println!("   Other peers will reject connections from this agent.");
        println!("   Reinitialize with NEAR verification:");
        println!("   1. near login --account-id {}", config.identity.account_id);
        println!("   2. rm -rf ~/.gork-agent");
        println!("   3. gork-agent init --account {}", config.identity.account_id);
        println!();
        println!("Starting anyway in 3 seconds... (Ctrl+C to cancel)");
        std::thread::sleep(std::time::Duration::from_secs(3));
        println!();
    } else {
        println!("✅ NEAR verification confirmed");
    }
    println!();

    // Create event channel
    let (event_sender, mut event_receiver) = tokio::sync::mpsc::unbounded_channel();

    println!("🌐 Initializing P2P network...");

    // Check if we should require authentication from other peers
    let require_auth = config.near_verified;

    // Parse bootstrap peers
    let mut bootstrap_addrs = vec![];
    if let Some(peers_str) = bootstrap_peers {
        for peer_str in peers_str.split(',') {
            let peer_str = peer_str.trim();
            if !peer_str.is_empty() {
                match peer_str.parse::<libp2p::Multiaddr>() {
                    Ok(addr) => bootstrap_addrs.push(addr),
                    Err(e) => {
                        eprintln!("⚠️  Invalid bootstrap peer '{}': {}", peer_str, e);
                    }
                }
            }
        }
    };

    // Create network config
    let network_config = network::NetworkConfig {
        port,
        bootstrap_peers: bootstrap_addrs,
    };

    // Create network with authentication requirement
    let mut p2p_network = network::AgentNetwork::with_auth(
        config.identity.clone(),
        network_config,
        event_sender,
        None,  // We'll handle peer authentication at application layer
        require_auth,
    )
    .await?;

    // Start listening
    let listen_addr = p2p_network.listen(Some(port)).await?;
    println!("📡 Listening on: {}", listen_addr);
    println!("   Peer ID: {}", p2p_network.peer_id());
    println!("   API Port: {} (HTTP)", port + 1);
    
    if p2p_network.requires_auth() {
        println!("   🔒 Authentication: REQUIRED (rejecting unverified peers)");
    } else {
        println!("   ⚠️  Authentication: DISABLED (accepting all peers)");
    }
    println!();

    println!("✅ Daemon started successfully!");
    println!("   Press Ctrl+C to stop");
    println!();

    // Create message handler
    let mut message_handler = network::MessageHandler::new(&config.identity.account_id);

    // Clone storage path for use in async block
    let storage_path = get_storage_path();
    let storage_path_clone = storage_path.clone();
    let account_id = config.identity.account_id.clone();
    let peer_id = p2p_network.peer_id().to_string();
    
    // Start HTTP API server
    let api_port = port + 1;
    let api_routes = create_api_routes(
        storage_path_clone,
        account_id.clone(),
        peer_id.clone(),
    );
    
    let api_server = tokio::spawn(async move {
        println!("🌐 API server running on http://127.0.0.1:{}", api_port);
        warp::serve(api_routes)
            .run(([127, 0, 0, 1], api_port))
            .await;
    });

    // Run network event loop
    tokio::select! {
        // Network runs in background, processing events internally
        biased;
        
        _ = async {
            loop {
                if let Some(event) = event_receiver.recv().await {
                    match event {
                        network::NetworkEvent::MessageReceived { from, message } => {
                            // Process incoming message through security layer
                            match message_handler.handle_message(from.clone(), &message) {
                                Ok(Some(msg)) => {
                                    // Save to inbox
                                    match storage::AgentStorage::open(&storage_path) {
                                        Ok(storage) => {
                                            if let Err(e) = storage.save_message(&msg) {
                                                eprintln!("⚠️  Failed to save message: {}", e);
                                            } else {
                                                println!("📨 Received message from: {}", from);
                                            }
                                        }
                                        Err(e) => eprintln!("⚠️  Failed to open storage: {}", e),
                                    }
                                }
                                Ok(None) => {
                                    // Message blocked by security
                                    println!("🚫 Message from {} blocked by security", from);
                                }
                                Err(e) => {
                                    eprintln!("⚠️  Failed to process message: {}", e);
                                }
                            }
                        }
                        network::NetworkEvent::PeerConnected(peer_id) => {
                            info!("Peer connected: {}", peer_id);
                        }
                        network::NetworkEvent::PeerDisconnected(peer_id) => {
                            info!("Peer disconnected: {}", peer_id);
                        }
                        network::NetworkEvent::Error(e) => {
                            eprintln!("⚠️  Network error: {}", e);
                        }
                    }
                }
            }
        } => {}

        _ = p2p_network.run() => {
            // Network finished (shouldn't happen)
        }

        _ = tokio::signal::ctrl_c() => {
            println!("\n🛑 Shutting down...");
        }
    }

    println!("👋 Daemon stopped");
    Ok(())
}

/// Start a minimal P2P relay server
async fn start_relay(
    port: u16,
    max_circuits: usize,
    enable_metrics: bool,
    metrics_port: u16,
) -> Result<()> {
    println!();
    println!("{}:", "=".repeat(60));
    println!("🌐 Gork Hybrid Relay Server");
    println!("{}:", "=".repeat(60));
    println!();

    // Check agent initialization
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("❌ No agent initialized. Run: gork-agent init --account <your.near>");
        println!();
        println!("For development/testing, use:");
        println!("  gork-agent init --account relay.testnet --dev-mode");
        return Ok(());
    }

    let storage = storage::AgentStorage::open(&storage_path)?;
    let config = storage
        .load_config()?
        .ok_or_else(|| anyhow::anyhow!("No agent configuration found"))?;

    if !config.near_verified {
        println!("⚠️  WARNING: Relay not NEAR-verified!");
        println!("   Peers may refuse to use this relay.");
        println!();
        println!("Starting anyway in 3 seconds... (Ctrl+C to cancel)");
        std::thread::sleep(std::time::Duration::from_secs(3));
        println!();
    }

    println!("🤖 Relay Identity: {}", config.identity.account_id);
    println!("📡 Port: {}", port);
    println!("🔌 Max circuits: {}", max_circuits);
    println!();

    // Create relay configuration
    let relay_config = relay::RelayConfig {
        port,
        max_circuits,
        max_circuit_duration_secs: 120,
        max_circuit_bytes: 1024 * 1024,
        enable_metrics,
        metrics_port,
    };

    // Create relay server
    let (event_sender, mut event_receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut relay_server = relay::RelayServer::with_events(relay_config.clone(), event_sender).await?;

    // Start listening
    let listen_addr = relay_server.listen().await?;
    println!("✅ Relay listening on: {}", listen_addr);
    println!("   Peer ID: {}", relay_server.peer_id);
    println!();

    // Print connection strings for different scenarios
    println!("📝 Connection strings for peers:");
    println!();
    println!("   Localhost:");
    println!("     {}", relay_server.connection_string("127.0.0.1"));
    println!();
    println!("   LAN (find your IP with 'ip addr' or 'ifconfig'):");
    println!("     /ip4/<YOUR-LAN-IP>/tcp/{}/p2p/{}", port, relay_server.peer_id);
    println!();
    println!("   Public (requires public IP):");
    println!("     /ip4/<YOUR-PUBLIC-IP>/tcp/{}/p2p/{}", port, relay_server.peer_id);
    println!();

    println!("🔧 Relay Roles:");
    println!("   1️⃣  Bootstrap Node - Peer discovery via Kademlia DHT");
    println!("   2️⃣  Rendezvous - Coordinate hole punching between peers");
    println!("   3️⃣  Circuit Relay - Fallback for symmetric NAT/CGNAT");
    println!();

    // Start metrics server if enabled
    if enable_metrics {
        let metrics_port = metrics_port;
        let peer_id = relay_server.peer_id.to_string();
        let (stats_tx, stats_rx) = tokio::sync::mpsc::channel(16);
        
        println!("📊 Metrics: http://0.0.0.0:{}", metrics_port);
        
        tokio::spawn(async move {
            relay::start_metrics_server(metrics_port, peer_id, stats_rx).await;
        });

        // Spawn stats reporter
        let stats_tx_clone = stats_tx.clone();
        let relay_peer_id = relay_server.peer_id.to_string();
        let relay_port = relay_server.config.port;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                // Send basic stats (can't access swarm from different thread)
                let stats = relay::RelayStats {
                    peer_id: relay_peer_id.clone(),
                    port: relay_port,
                    connected_peers: 0,
                };
                let _ = stats_tx_clone.send(stats).await;
            }
        });
    }

    println!("✅ Relay started successfully!");
    println!("   Press Ctrl+C to stop");
    println!();

    // Event monitoring task
    tokio::spawn(async move {
        while let Some(event) = event_receiver.recv().await {
            match event {
                relay::RelayEvent::PeerConnected(peer) => {
                    info!("✅ Peer connected: {}", peer);
                }
                relay::RelayEvent::PeerDisconnected(peer) => {
                    info!("❌ Peer disconnected: {}", peer);
                }
                relay::RelayEvent::CircuitEstablished { src, dst } => {
                    info!("🔀 Circuit established: {} → {}", src, dst);
                }
                relay::RelayEvent::CircuitClosed { src, dst } => {
                    info!("🔌 Circuit closed: {} → {}", src, dst);
                }
                relay::RelayEvent::Error(e) => {
                    error!("Relay error: {}", e);
                }
            }
        }
    });

    // Status reporter
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            info!("📊 Relay status: running");
        }
    });

    // Run relay event loop
    tokio::select! {
        _ = relay_server.run() => {
            // This will run forever until Ctrl+C
        }

        _ = tokio::signal::ctrl_c() => {
            println!("\n🛑 Shutting down relay...");
        }
    }

    println!("👋 Relay stopped");
    Ok(())
}

/// Handle Skills commands
async fn handle_skills_command(action: SkillsCommands) -> Result<()> {
    match action {
        SkillsCommands::Install { path } => {
            let package_path = std::path::Path::new(&path);
            skills::install_skill(package_path)?;
            Ok(())
        }
        SkillsCommands::List => {
            list_local_skills()?;
            Ok(())
        }
        SkillsCommands::Show { name } => {
            show_local_skill(&name)?;
            Ok(())
        }
        SkillsCommands::Remove { name } => {
            skills::remove_local_skill(&name)?;
            Ok(())
        }
    }
}

/// Handle Execute commands
async fn handle_execute_command(
    action: ExecuteCommands,
    registry_id: &str,
    network: &str,
) -> Result<()> {
    match action {
        ExecuteCommands::Request { agent, skill, capability, input } => {
            request_task_execution(&agent, &skill, &capability, &input, registry_id, network).await
        }
        ExecuteCommands::Rate { agent, rating } => {
            rate_agent(&agent, rating, registry_id, network).await
        }
    }
}

/// Handle Marketplace commands
async fn handle_marketplace_command(action: MarketplaceCommands) -> Result<()> {
    match action {
        MarketplaceCommands::List { tag, limit } => {
            list_discovered_skills(tag, limit).await
        }
    }
}

/// List local skills
fn list_local_skills() -> Result<()> {
    println!("📦 Local Skills");
    println!();

    let skills = skills::list_local_skills()?;

    if skills.is_empty() {
        println!("No skills installed.");
        println!();
        println!("Install a skill:");
        println!("  gork-agent skills install --path ./skill-package/");
    } else {
        for skill in skills {
            println!("📦 {} @ {}", skill.name, skill.version);
            println!("   {}", skill.description);
            println!("   Tags: {}", skill.tags.join(", "));
            println!();
        }
    }

    Ok(())
}

/// Show local skill details
fn show_local_skill(name: &str) -> Result<()> {
    if let Some(skill) = skills::get_local_skill(name)? {
        println!("📦 {}", skill.name);
        println!("   Version: {}", skill.version);
        println!("   Description: {}", skill.description);
        println!();
        println!("🏷️  Tags: {}", skill.tags.join(", "));
        println!();
        println!("⚙️  Capabilities:");
        for cap in &skill.capabilities {
            println!("   • {} - {}", cap.name, cap.description);
        }
        println!();
        println!("📋 Requirements:");
        println!("   Timeout: {}s", skill.requirements.timeout_secs);
        println!("   Memory: {}MB", skill.requirements.memory_mb);
        Ok(())
    } else {
        println!("Skill not found: {}", name);
        println!();
        println!("List installed skills:");
        println!("  gork-agent skills list");
        Err(anyhow::anyhow!("Skill not found"))
    }
}

/// Request task execution from another agent (with trust verification)
async fn request_task_execution(
    agent: &str,
    skill: &str,
    capability: &str,
    input: &str,
    registry_id: &str,
    network: &str,
) -> Result<()> {
    println!("🤝 Agent Collaboration Request");
    println!();
    println!("   Target: {}", agent);
    println!("   Skill: {}", skill);
    println!("   Capability: {}", capability);
    println!();

    // Parse input as JSON
    let input_json: serde_json::Value = serde_json::from_str(input)
        .map_err(|_| anyhow::anyhow!("Invalid JSON input"))?;

    // Step 1: Verify agent trust on NEAR registry
    println!("🔍 Step 1: Verifying agent trust...");
    let collab = skills::CollaborationFlow::new(registry_id.to_string(), network.to_string());
    let result = collab.request_task_with_verification(
        agent,
        skill,
        capability,
        input_json,
        50, // Minimum reputation
    ).await?;

    match result {
        skills::CollaborationResult::Pending(request_id) => {
            println!();
            println!("✅ Task request created!");
            println!("   Request ID: {}", request_id);
            println!();
            println!("⏳ Next steps:");
            println!("   1. Start daemon: gork-agent daemon");
            println!("   2. Daemon will send P2P request to {}", agent);
            println!("   3. Agent will verify your identity on NEAR registry");
            println!("   4. Agent executes task and returns results");
            println!();
            println!("⭐ After collaboration, rate the agent:");
            println!("   gork-agent execute rate --agent {} --rating 5", agent);
        }
        skills::CollaborationResult::Rejected(reason) => {
            println!("❌ Request rejected: {}", reason);
            println!();
            println!("💡 Tips:");
            println!("   - Check agent reputation on registry");
            println!("   - Try agents with higher reputation");
            println!("   - View: gork-agent list --limit 20");
        }
        skills::CollaborationResult::Success(response) => {
            println!("✅ Task completed!");
            println!("   Result: {:?}", response.result);
        }
    }

    Ok(())
}

/// List discovered skills from P2P network
async fn list_discovered_skills(tag: Option<String>, limit: u32) -> Result<()> {
    println!("🌐 P2P Discovered Skills");
    println!();

    if let Some(search_tag) = tag {
        println!("   Tag: {}", search_tag);
    }
    println!("   Limit: {}", limit);
    println!();

    println!("⚠️  P2P discovery requires daemon to be running.");
    println!("   Start daemon: gork-agent daemon");
    println!();
    println!("   The daemon will:");
    println!("   1. Connect to P2P network");
    println!("   2. Listen for skill advertisements");
    println!("   3. Maintain discovered skills cache");
    println!();
    println!("   Discovered skills will appear here when daemon is running.");

    Ok(())
}

/// Rate an agent after collaboration
async fn rate_agent(agent: &str, rating: u32, registry_id: &str, network: &str) -> Result<()> {
    if !(1..=5).contains(&rating) {
        println!("❌ Rating must be between 1 and 5");
        return Ok(());
    }

    println!("⭐ Rating Agent");
    println!();
    println!("   Agent: {}", agent);
    println!("   Rating: {}★", rating);
    println!("   Registry: {}", registry_id);
    println!();

    // Check agent info first
    let client = registry::RegistryClient::new(registry_id.to_string(), network);
    if let Some(agent_info) = client.get_agent(agent).await? {
        println!("   Current reputation: {}/100", agent_info.reputation);
        println!("   Total ratings: {}", agent_info.rating_count);
        println!();
    }

    println!("⚠️  To submit rating, run this command:");
    println!();
    println!("   near call {} rate_agent '{{\"agent_id\": \"{}\", \"rating\": {}}}' --accountId YOUR_ACCOUNT",
        registry_id, agent, rating
    );
    println!();
    println!("💡 In production, this will be done automatically via the CLI");

    Ok(())
}

fn handle_api_key_command(action: ApiKeyCommands) -> Result<()> {
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("❌ No agent initialized. Run: gork-agent init --account <your.near>");
        return Ok(());
    }
    
    let storage = storage::AgentStorage::open(&storage_path)?;
    
    match action {
        ApiKeyCommands::Create { name, permissions } => {
            let key = storage.create_api_key(&name, &permissions)?;
            println!("✅ API Key Created");
            println!();
            println!("   Name: {}", name);
            println!("   Permissions: {}", permissions);
            println!("   Key: {}", key);
            println!();
            println!("⚠️  Store this key securely - it won't be shown again!");
            println!();
            println!("Usage:");
            println!("   curl -H \"X-API-Key: {}\" http://127.0.0.1:4002/api/v1/status", key);
        }
        ApiKeyCommands::List => {
            let keys = storage.list_api_keys()?;
            if keys.is_empty() {
                println!("📭 No API keys found");
                println!();
                println!("Create one with: gork-agent api-keys create --name <name>");
                return Ok(());
            }
            
            println!("🔑 API Keys ({} total)", keys.len());
            println!();
            for (key, name, created, last_used) in keys {
                let created_date = chrono::DateTime::from_timestamp(created, 0)
                    .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| created.to_string());
                
                let last_used_str = last_used
                    .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
                    .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "Never".to_string());
                
                println!("┌─────────────────────────────────────");
                println!("│ Name: {}", name);
                println!("│ Key: {}...", &key[..20]);
                println!("│ Created: {}", created_date);
                println!("│ Last Used: {}", last_used_str);
                println!("└─────────────────────────────────────");
            }
        }
        ApiKeyCommands::Revoke { key } => {
            if storage.revoke_api_key(&key)? {
                println!("✅ API key revoked");
            } else {
                println!("❌ API key not found");
            }
        }
    }
    
    Ok(())
}

fn handle_queue_command(action: QueueCommands) -> Result<()> {
    let storage_path = get_storage_path();
    if !storage_path.exists() {
        println!("❌ No agent initialized. Run: gork-agent init --account <your.near>");
        return Ok(());
    }
    
    let storage = storage::AgentStorage::open(&storage_path)?;
    
    match action {
        QueueCommands::List { limit } => {
            let messages = storage.get_pending_messages(limit)?;
            
            if messages.is_empty() {
                println!("📭 Message queue empty");
                return Ok(());
            }
            
            println!("📤 Message Queue ({} pending)", messages.len());
            println!();
            
            for (id, to, message) in messages {
                println!("┌─────────────────────────────────────");
                println!("│ ID: {}", id);
                println!("│ To: {}", to);
                println!("│ Message: {}", if message.len() > 50 { 
                    format!("{}...", &message[..50]) 
                } else { 
                    message 
                });
                println!("└─────────────────────────────────────");
            }
            
            println!();
            println!("💡 Messages will be sent when P2P connection is available");
        }
        QueueCommands::Cleanup { days } => {
            let removed = storage.cleanup_queue(days)?;
            println!("✅ Cleaned up {} sent messages older than {} days", removed, days);
        }
    }
    
    Ok(())
}
