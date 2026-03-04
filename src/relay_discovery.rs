use anyhow::Result;
use std::process::Command;

/// Relay discovery via DNS TXT records (simplified version)
pub struct RelayDiscovery {
    dns_contract: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct DnsRecord {
    pub record_type: String,
    pub value: String,
    pub ttl: u32,
}

/// Hardcoded fallback relays for DNS failure/poisoning protection
/// These are trusted relays that can be used when DNS is unavailable or compromised
const FALLBACK_RELAYS: &[(&str, &str)] = &[
    // Primary Railway relay
    ("gork-relay-production.up.railway.app", "/dns4/gork-relay-production.up.railway.app/tcp/443/wss/p2p/12D3KooWA9CMq2VYF5dt6TvWGPKKyXEwnp5Q2zwGtmb7XAu2Z8fG"),
    // Backup relay (TODO: deploy additional relays for redundancy)
    // ("relay-backup.example.com", "/dns4/relay-backup.example.com/tcp/4001/p2p/PEER_ID"),
];

/// Known trusted peer IDs for relay validation
/// If DNS returns a different peer ID, it may be poisoned
const TRUSTED_RELAY_PEERS: &[&str] = &[
    "12D3KooWA9CMq2VYF5dt6TvWGPKKyXEwnp5Q2zwGtmb7XAu2Z8fG", // Primary relay
];

impl RelayDiscovery {
    pub fn new(dns_contract: String) -> Self {
        Self { dns_contract }
    }

    /// Discover relay multiaddr from domain using NEAR CLI
    /// 
    /// Example:
    ///   relay.jemartel.near → queries _p2p.relay.jemartel.near TXT
    ///   Returns: /dns4/relay.jemartel.near/tcp/4001/p2p/<PEER_ID>
    pub async fn discover(&self, domain: &str) -> Result<String> {
        // Try DNS discovery first
        match self.discover_via_dns(domain).await {
            Ok(multiaddr) => {
                // Validate peer ID to detect DNS poisoning
                if self.validate_relay_peer(&multiaddr) {
                    return Ok(multiaddr);
                } else {
                    eprintln!("⚠️  DNS returned untrusted peer ID, using fallback");
                }
            }
            Err(e) => {
                eprintln!("⚠️  DNS discovery failed: {}, using fallback", e);
            }
        }
        
        // Fall back to hardcoded relays
        self.get_fallback_relay()
    }
    
    /// Discover relay via DNS TXT records
    async fn discover_via_dns(&self, domain: &str) -> Result<String> {
        // Extract base domain (e.g., relay.jemartel.near → jemartel.near)
        let parts: Vec<&str> = domain.split('.').collect();
        if parts.len() < 3 {
            anyhow::bail!("Invalid domain format. Expected: subdomain.account.tld");
        }

        // Construct DNS contract address
        let tld = parts.last().unwrap();
        let account = parts[parts.len() - 2];
        let subdomain = parts[0];
        
        let dns_contract = format!("dns.{}.{}", account, tld);
        
        // Query TXT record using NEAR CLI
        let txt_name = format!("_p2p.{}", subdomain);
        let multiaddr = self.query_txt_record_via_cli(&dns_contract, &txt_name)?;
        
        Ok(multiaddr)
    }
    
    /// Validate relay peer ID against trusted list
    fn validate_relay_peer(&self, multiaddr: &str) -> bool {
        // Extract peer ID from multiaddr (format: /p2p/PEER_ID)
        if let Some(peer_start) = multiaddr.find("/p2p/") {
            let peer_id = &multiaddr[peer_start + 5..];
            let peer_id = peer_id.split('/').next().unwrap_or("");
            
            // Check if peer ID is in trusted list
            // Note: If TRUSTED_RELAY_PEERS is empty, accept all (dev mode)
            if TRUSTED_RELAY_PEERS.is_empty() {
                return true;
            }
            
            return TRUSTED_RELAY_PEERS.contains(&peer_id);
        }
        
        false
    }
    
    /// Get fallback relay (first available)
    fn get_fallback_relay(&self) -> Result<String> {
        if let Some((_, multiaddr)) = FALLBACK_RELAYS.first() {
            Ok(multiaddr.to_string())
        } else {
            anyhow::bail!("No fallback relays configured")
        }
    }

    /// Query NEAR DNS contract for TXT record using NEAR CLI
    fn query_txt_record_via_cli(&self, contract: &str, name: &str) -> Result<String> {
        let output = Command::new("near")
            .args([
                "view",
                contract,
                "dns_query",
                &format!(r#"{{"name":"{}","record_type":"TXT"}}"#, name),
                "--networkId",
                "mainnet",
            ])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to query DNS: {}", String::from_utf8_lossy(&output.stderr));
        }

        let response = String::from_utf8_lossy(&output.stdout);
        
        // Parse JSON array of records
        // Expected format: [ { "record_type": "TXT", "value": "...", "ttl": 300 } ]
        let records: Vec<DnsRecord> = serde_json::from_str(&response)
            .map_err(|e| anyhow::anyhow!("Failed to parse DNS response: {}", e))?;

        if let Some(record) = records.first() {
            if record.record_type == "TXT" {
                return Ok(record.value.clone());
            }
        }

        anyhow::bail!("No TXT record found for {}", name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires NEAR CLI to be installed
    async fn test_discover_relay() {
        let discovery = RelayDiscovery::new("dns.jemartel.near".to_string());
        let multiaddr = discovery.discover("relay.jemartel.near").await.unwrap();
        
        assert!(multiaddr.contains("relay.jemartel.near"));
        assert!(multiaddr.contains("12D3KooWA9CMq2VYF5dt6TvWGPKKyXEwnp5Q2zwGtmb7XAu2Z8fG"));
    }
}
