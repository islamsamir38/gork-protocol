//! NEAR DNS Resolver Module
//!
//! Decentralized DNS resolution via NEAR blockchain smart contracts.
//! Enables peer discovery without hardcoded IPs or centralized DNS.
//!
//! # Example
//! ```
//! let resolver = NearDnsResolver::new(Network::Mainnet);
//! let ip = resolver.resolve_ip("gork.jemartel.near").await?;
//! // Returns: "192.168.1.50"
//! ```
//!
//! # How It Works
//! ```
//! gork.jemartel.near
//!       │
//!       ▼
//! Query: dns.jemartel.near.dns_query("gork", "A")
//!       │
//!       ▼
//! Returns: [{ record_type: "A", value: "192.168.1.50", ttl: 300 }]
//! ```

use anyhow::{anyhow, Result};
use base64::{Engine as _, engine::general_purpose};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

use crate::auth::Network;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Default DNS cache TTL (5 minutes)
const DEFAULT_CACHE_TTL_SECS: u64 = 300;

/// RPC timeout in seconds
const RPC_TIMEOUT_SECS: u64 = 10;

// ============================================================================
// DNS RECORD STRUCTURES
// ============================================================================

/// DNS record returned from NEAR DNS contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearDnsRecord {
    pub record_type: String,
    pub value: String,
    pub ttl: u64,
    pub priority: Option<u64>,
}

impl NearDnsRecord {
    /// Check if this is an A record (IPv4)
    pub fn is_a(&self) -> bool {
        self.record_type == "A"
    }

    /// Check if this is an AAAA record (IPv6)
    pub fn is_aaaa(&self) -> bool {
        self.record_type == "AAAA"
    }

    /// Check if this is a TXT record
    pub fn is_txt(&self) -> bool {
        self.record_type == "TXT"
    }

    /// Check if this is a CNAME record
    pub fn is_cname(&self) -> bool {
        self.record_type == "CNAME"
    }
}

/// Cached DNS entry with timestamp
#[derive(Debug, Clone)]
struct CachedRecord {
    records: Vec<NearDnsRecord>,
    cached_at: u64,
}

// ============================================================================
// DNS RESOLVER
// ============================================================================

/// NEAR DNS resolver for decentralized peer discovery
pub struct NearDnsResolver {
    http_client: Client,
    network: Network,
    cache: HashMap<String, CachedRecord>,
    cache_ttl_secs: u64,
}

impl NearDnsResolver {
    /// Create a new DNS resolver for the given network
    pub fn new(network: Network) -> Self {
        Self {
            http_client: Client::new(),
            network,
            cache: HashMap::new(),
            cache_ttl_secs: DEFAULT_CACHE_TTL_SECS,
        }
    }

    /// Set custom cache TTL
    pub fn with_cache_ttl(mut self, ttl_secs: u64) -> Self {
        self.cache_ttl_secs = ttl_secs;
        self
    }

    /// Resolve a domain name via NEAR DNS contract
    /// 
    /// # Example
    /// ```
    /// let records = resolver.resolve("gork.jemartel.near", "A").await?;
    /// for record in records {
    ///     println!("IP: {}", record.value);
    /// }
    /// ```
    pub async fn resolve(&mut self, domain: &str, record_type: &str) -> Result<Vec<NearDnsRecord>> {
        let now = current_timestamp();
        let cache_key = format!("{}:{}", domain.to_lowercase(), record_type.to_uppercase());

        // Check cache
        if let Some(cached) = self.cache.get(&cache_key) {
            if now - cached.cached_at < self.cache_ttl_secs {
                info!("DNS cache hit for {} {}", record_type, domain);
                return Ok(cached.records.clone());
            }
        }

        // Parse domain to get contract and name
        let (dns_contract, name) = self.parse_domain(domain)?;

        // Query the DNS contract
        let records = self.query_dns_contract(&dns_contract, &name, record_type).await?;

        // Cache the result
        if !records.is_empty() {
            self.cache.insert(cache_key, CachedRecord {
                records: records.clone(),
                cached_at: now,
            });
        }

        info!("Resolved {} {} → {} records via {}", 
              record_type, domain, records.len(), dns_contract);
        Ok(records)
    }

    /// Resolve all record types for a domain
    pub async fn resolve_all(&mut self, domain: &str) -> Result<Vec<NearDnsRecord>> {
        let (dns_contract, name) = self.parse_domain(domain)?;
        self.query_dns_contract_all(&dns_contract, &name).await
    }

    /// Get first A record IP address for a domain
    pub async fn resolve_ip(&mut self, domain: &str) -> Result<Option<String>> {
        let records = self.resolve(domain, "A").await?;
        Ok(records.iter().find(|r| r.is_a()).map(|r| r.value.clone()))
    }

    /// Get all A record IPs for a domain (useful for load balancing)
    pub async fn resolve_ips(&mut self, domain: &str) -> Result<Vec<String>> {
        let records = self.resolve(domain, "A").await?;
        Ok(records.iter().filter(|r| r.is_a()).map(|r| r.value.clone()).collect())
    }

    /// Get TXT record value
    pub async fn resolve_txt(&mut self, domain: &str) -> Result<Option<String>> {
        let records = self.resolve(domain, "TXT").await?;
        Ok(records.iter().find(|r| r.is_txt()).map(|r| r.value.clone()))
    }

    /// Get CNAME target
    pub async fn resolve_cname(&mut self, domain: &str) -> Result<Option<String>> {
        let records = self.resolve(domain, "CNAME").await?;
        Ok(records.iter().find(|r| r.is_cname()).map(|r| r.value.clone()))
    }

    /// Parse domain like "gork.jemartel.near" into ("dns.jemartel.near", "gork")
    fn parse_domain(&self, domain: &str) -> Result<(String, String)> {
        let parts: Vec<&str> = domain.trim_end_matches('.').split('.').collect();
        
        // Need at least: account.tld (2 parts) for root record
        if parts.len() < 2 {
            return Err(anyhow!(
                "Invalid NEAR domain '{}': expected format 'account.tld' or 'name.account.tld'",
                domain
            ));
        }

        // Validate TLD
        let tld = *parts.last().unwrap();
        if !self.is_valid_near_tld(tld) {
            return Err(anyhow!(
                "Invalid NEAR TLD '.{}': supported TLDs are .near, .testnet, .aurora, etc.",
                tld
            ));
        }

        // Build DNS contract: dns.{account}.{tld}
        let account = parts[parts.len() - 2];
        let dns_contract = format!("dns.{}.{}", account, tld);
        
        // Name is everything before the account (empty string for root record)
        let name = if parts.len() == 2 {
            "".to_string()  // Root record (@)
        } else {
            parts[..parts.len() - 2].join(".")
        };

        Ok((dns_contract, name))
    }

    /// Check if TLD is a valid NEAR TLD
    fn is_valid_near_tld(&self, tld: &str) -> bool {
        matches!(tld, "near" | "testnet" | "aurora" | "tg" | "sweat" | "kaiching" | "sharddog")
    }

    /// Query DNS contract for specific record type
    async fn query_dns_contract(
        &self,
        contract: &str,
        name: &str,
        record_type: &str,
    ) -> Result<Vec<NearDnsRecord>> {
        let args = serde_json::json!({
            "name": name,
            "record_type": record_type
        });
        
        let args_base64 = general_purpose::STANDARD
            .encode(serde_json::to_vec(&args)?);

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "call_function",
                "finality": "final",
                "account_id": contract,
                "method_name": "dns_query",
                "args_base64": args_base64
            }
        });

        let response = self.http_client
            .post(self.network.rpc_url())
            .json(&body)
            .timeout(std::time::Duration::from_secs(RPC_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| anyhow!("DNS query failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("DNS RPC error: {}", response.status()));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse DNS response: {}", e))?;

        self.parse_dns_result(&result)
    }

    /// Query DNS contract for all records of a name
    async fn query_dns_contract_all(
        &self,
        contract: &str,
        name: &str,
    ) -> Result<Vec<NearDnsRecord>> {
        let args = serde_json::json!({ "name": name });
        let args_base64 = general_purpose::STANDARD
            .encode(serde_json::to_vec(&args)?);

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "call_function",
                "finality": "final",
                "account_id": contract,
                "method_name": "dns_query_all",
                "args_base64": args_base64
            }
        });

        let response = self.http_client
            .post(self.network.rpc_url())
            .json(&body)
            .timeout(std::time::Duration::from_secs(RPC_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| anyhow!("DNS query failed: {}", e))?;

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse DNS response: {}", e))?;

        self.parse_dns_result(&result)
    }

    /// Parse DNS result from NEAR RPC response
    fn parse_dns_result(&self, result: &serde_json::Value) -> Result<Vec<NearDnsRecord>> {
        // Extract result bytes from NEAR RPC response
        let result_bytes = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.as_array())
            .ok_or_else(|| anyhow!("No result in DNS response"))?;

        // Convert to bytes
        let bytes: Vec<u8> = result_bytes
            .iter()
            .filter_map(|b| b.as_u64().map(|v| v as u8))
            .collect();

        // Empty result
        if bytes.is_empty() || bytes == vec![0] {
            return Ok(vec![]);
        }

        // Try parsing as Vec<NearDnsRecord>
        match serde_json::from_slice::<Vec<NearDnsRecord>>(&bytes) {
            Ok(records) => Ok(records),
            Err(_) => {
                // Try parsing as single NearDnsRecord
                match serde_json::from_slice::<NearDnsRecord>(&bytes) {
                    Ok(record) => Ok(vec![record]),
                    Err(e) => {
                        warn!("Failed to parse DNS records: {}", e);
                        Ok(vec![])
                    }
                }
            }
        }
    }

    /// List all DNS names in a contract
    pub async fn list_names(&self, contract: &str) -> Result<Vec<String>> {
        let args = serde_json::json!({});
        let args_base64 = general_purpose::STANDARD
            .encode(serde_json::to_vec(&args)?);

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "dontcare",
            "method": "query",
            "params": {
                "request_type": "call_function",
                "finality": "final",
                "account_id": contract,
                "method_name": "dns_list_names",
                "args_base64": args_base64
            }
        });

        let response = self.http_client
            .post(self.network.rpc_url())
            .json(&body)
            .timeout(std::time::Duration::from_secs(RPC_TIMEOUT_SECS))
            .send()
            .await
            .map_err(|e| anyhow!("DNS list failed: {}", e))?;

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse DNS response: {}", e))?;

        let result_bytes = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.as_array())
            .ok_or_else(|| anyhow!("No names found"))?;

        let bytes: Vec<u8> = result_bytes
            .iter()
            .filter_map(|b| b.as_u64().map(|v| v as u8))
            .collect();

        if bytes.is_empty() {
            return Ok(vec![]);
        }

        serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("Failed to parse names: {}", e))
    }

    /// Clear the DNS cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        info!("DNS cache cleared");
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        let total = self.cache.len();
        let valid = self.cache.values()
            .filter(|c| current_timestamp() - c.cached_at < self.cache_ttl_secs)
            .count();
        (valid, total)
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Get current timestamp in seconds
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_domain() {
        let resolver = NearDnsResolver::new(Network::Mainnet);

        // Standard domain
        let (contract, name) = resolver.parse_domain("gork.jemartel.near").unwrap();
        assert_eq!(contract, "dns.jemartel.near");
        assert_eq!(name, "gork");

        // Subdomain
        let (contract, name) = resolver.parse_domain("node1.gork.jemartel.near").unwrap();
        assert_eq!(contract, "dns.jemartel.near");
        assert_eq!(name, "node1.gork");

        // Root record (@)
        let (contract, name) = resolver.parse_domain("jemartel.near").unwrap();
        assert_eq!(contract, "dns.jemartel.near");
        assert_eq!(name, "");

        // Invalid domain
        assert!(resolver.parse_domain("invalid").is_err());
        assert!(resolver.parse_domain("invalid.com").is_err());
    }

    #[test]
    fn test_valid_tlds() {
        let resolver = NearDnsResolver::new(Network::Mainnet);

        assert!(resolver.is_valid_near_tld("near"));
        assert!(resolver.is_valid_near_tld("testnet"));
        assert!(resolver.is_valid_near_tld("aurora"));
        assert!(!resolver.is_valid_near_tld("com"));
        assert!(!resolver.is_valid_near_tld("org"));
    }

    #[test]
    fn test_dns_record_types() {
        let a_record = NearDnsRecord {
            record_type: "A".to_string(),
            value: "192.168.1.1".to_string(),
            ttl: 300,
            priority: None,
        };
        assert!(a_record.is_a());
        assert!(!a_record.is_txt());

        let txt_record = NearDnsRecord {
            record_type: "TXT".to_string(),
            value: "peer_id=12D3Koo...".to_string(),
            ttl: 300,
            priority: None,
        };
        assert!(txt_record.is_txt());
        assert!(!txt_record.is_a());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let mut resolver = NearDnsResolver::new(Network::Mainnet);
        
        let (valid, total) = resolver.cache_stats();
        assert_eq!(valid, 0);
        assert_eq!(total, 0);

        // Add to cache manually
        resolver.cache.insert("test:A".to_string(), CachedRecord {
            records: vec![],
            cached_at: current_timestamp(),
        });

        let (valid, total) = resolver.cache_stats();
        assert_eq!(valid, 1);
        assert_eq!(total, 1);
    }
}
