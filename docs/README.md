# Gork Agent Protocol - Documentation

This directory contains comprehensive documentation for the Gork Agent Protocol.

## Architecture & Design

### [RELAY_DESIGN.md](RELAY_DESIGN.md)
**Full relay architecture and design document**

- What is a relay and why you need one
- Architecture overview
- Protocol details (libp2p relay, NAT traversal)
- Deployment scenarios (bare metal, Docker, Kubernetes)
- Configuration options
- Security considerations
- Performance and resource requirements

### [RELAY_QUICKSTART.md](RELAY_QUICKSTART.md)
**Quick start guide for relay deployment**

- Prerequisites
- Basic relay setup
- Docker deployment
- Docker Compose setup
- Cloud deployment (DigitalOcean, Kubernetes)
- Health checks and metrics
- Production checklist
- Troubleshooting

## Test Results

### [RELAY_TEST_RESULTS.md](RELAY_TEST_RESULTS.md)
**End-to-end relay test results**

- Test setup and configuration
- Detailed test results
- Connection verification
- Known issues and fixes
- Summary and next steps

## Security & Authentication

### [MANDATORY_NEAR_VERIFICATION.md](MANDATORY_NEAR_VERIFICATION.md)
**Security model: mandatory NEAR verification**

- Why NEAR verification is mandatory
- Threat model
- Verification flow
- Dev mode vs production
- Security best practices

### [PEER_AUTHENTICATION.md](PEER_AUTHENTICATION.md)
**Peer authentication protocol**

- Authentication overview
- NEAR identity integration
- Credential verification
- Session management

### [MESSAGE_SECURITY.md](MESSAGE_SECURITY.md)
**Message encryption and security**

- Encryption algorithms (X25519, ChaCha20-Poly1305)
- Key derivation (HKDF-SHA256)
- Message format
- Signature verification (Ed25519)

### [SECURITY.md](SECURITY.md)
**Complete security overview**

- Threat model
- Security guarantees
- Cryptographic primitives
- Best practices
- Known limitations

## NEAR Integration

### [NEAR_LOGIN.md](NEAR_LOGIN.md)
**NEAR authentication setup**

- Setting up NEAR CLI
- Login flow
- Account verification
- Troubleshooting

## Additional Documentation

### [P2P_TEST_GUIDE.md](P2P_TEST_GUIDE.md)
**Guide to P2P testing**

- Testing methodology
- Test scenarios
- Debugging P2P issues

### [TWO_AGENT_VERIFICATION.md](TWO_AGENT_VERIFICATION.md)
**Two-agent verification protocol**

- Agent-to-agent communication
- Verification handshake
- Trust establishment

### [VERIFICATION_REPORT.md](VERIFICATION_REPORT.md)
**Verification and testing report**

- Test coverage
- Verification results
- Performance metrics

## Quick Links

### For Users
1. **Quick Start** → [RELAY_QUICKSTART.md](RELAY_QUICKSTART.md)
2. **Test Results** → [RELAY_TEST_RESULTS.md](RELAY_TEST_RESULTS.md)
3. **NEAR Setup** → [NEAR_LOGIN.md](NEAR_LOGIN.md)

### For Developers
1. **Architecture** → [RELAY_DESIGN.md](RELAY_DESIGN.md)
2. **Security** → [SECURITY.md](SECURITY.md)
3. **Testing** → [P2P_TEST_GUIDE.md](P2P_TEST_GUIDE.md)

### For Operators
1. **Deployment** → [RELAY_QUICKSTART.md](RELAY_QUICKSTART.md) (Cloud/Docker sections)
2. **Production Checklist** → [RELAY_QUICKSTART.md](RELAY_QUICKSTART.md) (Production checklist)
3. **Health Checks** → [RELAY_QUICKSTART.md](RELAY_QUICKSTART.md) (Checking relay status)

## Reading Order

### New Users
1. README.md (project root)
2. RELAY_QUICKSTART.md
3. RELAY_TEST_RESULTS.md
4. MANDATORY_NEAR_VERIFICATION.md

### Developers
1. README.md (project root)
2. RELAY_DESIGN.md
3. SECURITY.md
4. PEER_AUTHENTICATION.md
5. MESSAGE_SECURITY.md

### Operators
1. RELAY_QUICKSTART.md
2. RELAY_DESIGN.md (architecture overview)
3. SECURITY.md (security considerations)
4. RELAY_TEST_RESULTS.md (what to expect)

## Status Legend

- ✅ **Implemented** - Feature is complete and tested
- 🚧 **In Progress** - Feature under development
- 📋 **Planned** - Feature planned for future release
- ⚠️ **Experimental** - Feature may change
