use gork_agent::dns::NearDnsResolver;
use gork_agent::auth::Network;

#[tokio::main]
async fn main() {
    println!("Testing NEAR DNS Resolver...");
    
    let mut resolver = NearDnsResolver::new(Network::Mainnet);
    
    // Test 1: Resolve a NEAR domain
    println!("\n=== Test 1: Resolve gork.jemartel.near ===");
    match resolver.resolve("gork.jemartel.near", "A").await {
        Ok(records) => {
            if records.is_empty() {
                println!("⚠️  No A records found (expected if not configured)");
            } else {
                println!("✅ Found {} A record(s)", records.len());
                for r in records {
                    println!("   {} -> {} (TTL: {}s)", r.record_type, r.value, r.ttl);
                }
            }
        }
        Err(e) => {
            println!("❌ Error: {}", e);
        }
    }
    
    // Test 2: Resolve TXT record
    println!("\n=== Test 2: Resolve TXT record ===");
    match resolver.resolve_txt("gork.jemartel.near").await {
        Ok(Some(txt)) => println!("✅ TXT: {}", txt),
        Ok(None) => println!("⚠️  No TXT record"),
        Err(e) => println!("❌ Error: {}", e),
    }
    
    // Test 3: Resolve all records
    println!("\n=== Test 3: Resolve all records ===");
    match resolver.resolve_all("gork.jemartel.near").await {
        Ok(records) => {
            println!("✅ Found {} total record(s)", records.len());
            for r in records {
                println!("   {} -> {}", r.record_type, r.value);
            }
        }
        Err(e) => {
            println!("❌ Error: {}", e);
        }
    }
    
    // Test 4: Cache stats
    println!("\n=== Test 4: Cache Stats ===");
    let (hits, misses) = resolver.cache_stats();
    println!("Cache hits: {}, misses: {}", hits, misses);
    
    // Test 5: Invalid domain (should fail on resolve)
    println!("\n=== Test 5: Invalid domain ===");
    match resolver.resolve("invalid.domain", "A").await {
        Ok(_) => println!("❌ Should have failed"),
        Err(e) => println!("✅ Correctly rejected: {}", e),
    }
    
    // Test 6: Valid TLDs (try to resolve)
    println!("\n=== Test 6: Valid TLDs ===");
    for domain in &["test.near", "test.testnet"] {
        match resolver.resolve(domain, "A").await {
            Ok(records) => println!("✅ {} → {} records", domain, records.len()),
            Err(e) => println!("⚠️  {} → {} (expected if not configured)", domain, e),
        }
    }
    
    println!("\n=== Tests Complete ===");
}
