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
