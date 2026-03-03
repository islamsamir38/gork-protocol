# Web of Trust Deployment Debug Notes

## Problem
Contract deploys successfully but throws `PrepareError(Deserialization)` on any view call.

## Steps Taken
1. ✅ Removed std::collections (HashMap, HashSet) - incompatible with NEAR wasm
2. ✅ Replaced with Vec-based visited tracking
3. ✅ Removed TrustGraph struct (only kept data types)
4. ✅ Deployed to fresh account (registry-wot.testnet) - still fails

## Hypothesis
The issue is NOT:
- Old contract state (fresh account fails too)
- std::collections (all removed)

The issue MIGHT be:
- Borsh serialization issue with complex nested types
- near-sdk version mismatch
- Missing required features

## Next Steps
1. Try deploying minimal version without Web of Trust
2. Check near-sdk version (Cargo.toml says 5.17.0, but 5.24.1 was compiled)
3. Look at working NEAR contracts for comparison

## Files
- Contract: `/Users/asil/.openclaw/workspace/gork-protocol/contracts/registry/src/lib.rs`
- Trust module: `/Users/asil/.openclaw/workspace/gork-protocol/contracts/registry/src/trust.rs`
- Deployed to: `registry-wot.testnet`
