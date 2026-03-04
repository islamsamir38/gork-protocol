# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed - Mar 3, 2026

#### SQLite Migration (Major)

**Migrated from RocksDB to SQLite across all storage modules:**
- `AgentStorage` (messages, config, identity)
- `RateLimiter` (rate limit persistence)
- `AuditLog` (audit entry persistence)
- `PeerAuthenticator` (trusted peers)

**Why:**
- RocksDB locks prevented concurrent access from daemon and CLI
- SQLite with WAL mode allows multiple readers + one writer
- No more "database locked" errors when checking inbox while daemon runs

**Implementation:**
- All modules use `Arc<Mutex<Connection>>` for thread-safe access
- WAL mode enabled for better concurrent performance
- 5-second busy timeout for lock retries
- Fixed Rust borrow checker issues by collecting query results before processing

**Files Changed:**
- `src/storage/mod.rs` - Main storage migration
- `src/security/mod.rs` - Rate limiter + audit log
- `src/auth.rs` - Peer authentication
- `Cargo.toml` - Added rusqlite, removed rocksdb

#### Bug Fixes

- **Fixed blockchain registration** - Hex string → byte array conversion
- **Fixed NEAR key decoding** - 64→32 byte extraction for keypair format
- **Fixed register command** - Use correct contract (`registry-variant-c.testnet`)
- **Fixed daemon event loop** - Process incoming messages and save to inbox
- **Added `GORK_AGENT_HOME`** - Support multiple agents on same machine

### Testing

- ✅ End-to-end P2P messaging tested
- ✅ Concurrent access verified (daemon + CLI)
- ✅ Message persistence confirmed

### Documentation

- Updated README.md storage section
- Updated SKILLS_ARCHITECTURE.md
- Updated RELAY_SUCCESS.md

## [0.1.0] - 2026-02-XX

### Added

- Initial P2P networking with libp2p
- NEAR blockchain identity verification
- End-to-end encrypted messaging
- Agent Skills support
- Registry smart contract
- CLI interface
