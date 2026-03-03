// Test relay discovery directly
use std::process::Command;

fn main() {
    println!("🧪 Testing relay discovery...\n");

    let dns_contract = "dns.jemartel.near";
    let name = "_p2p.relay";
    
    println!("Querying: {}.{} TXT record", name, dns_contract);
    
    let output = Command::new("near")
        .args([
            "view",
            dns_contract,
            "dns_query",
            &format!(r#"{{"name":"{}","record_type":"TXT"}}"#, name),
            "--networkId",
            "mainnet",
        ])
        .output()
        .expect("Failed to execute near CLI");

    if output.status.success() {
        let response = String::from_utf8_lossy(&output.stdout);
        println!("✅ DNS Response:\n{}", response);
        
        // Parse the multiaddr
        if response.contains("/dns4/relay.jemartel.near") {
            println!("\n🎉 SUCCESS! Relay discovery working!");
            println!("Multiaddr found in TXT record");
        }
    } else {
        println!("❌ Failed: {}", String::from_utf8_lossy(&output.stderr));
    }
}
