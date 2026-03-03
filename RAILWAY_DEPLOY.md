# Gork P2P Relay - Railway Deployment

**Deployed:** Mar 3, 2026
**Status:** Ready for Railway

---

## Quick Deploy

[![Deploy on Railway](https://railway.app/button.svg)](https://railway.app/template/xxxxx)

Or deploy manually:

```bash
# Install Railway CLI
npm install -g @railway/cli

# Login
railway login

# Initialize project
cd /Users/asil/.openclaw/workspace/gork-protocol
railway init

# Deploy
railway up
```

---

## Environment Variables

No required environment variables. Optional:

```bash
RUST_LOG=info           # Logging level
NEAR_ACCOUNT=relay.gork # Optional NEAR account
```

---

## Ports

- **4001** - P2P relay (TCP+UDP)
- **9090** - Metrics endpoint (HTTP)

Railway will automatically assign a public URL.

---

## Configuration

The relay starts with these defaults:
- Max circuits: 1000
- Metrics: enabled
- Port: 4001

---

## After Deployment

1. Get your Railway URL:
   ```bash
   railway domain
   # Output: gork-relay.up.railway.app
   ```

2. Connect agents to relay:
   ```bash
   gork-agent daemon --bootstrap-peers /dns4/gork-relay.up.railway.app/tcp/4001/p2p/<PEER_ID>
   ```

3. Check health:
   ```bash
   curl https://gork-relay.up.railway.app:9090/health
   ```

---

## Monitoring

View logs:
```bash
railway logs
```

Metrics available at:
```
https://your-app.up.railway.app:9090/metrics
```

---

## Cost Estimate

Railway pricing:
- **Hobby plan:** $5/month (includes 500 hours)
- **Pro plan:** $20/month

Expected usage: ~$3-5/month for relay

---

## Troubleshooting

### Relay not accessible
- Check if port 4001 is exposed: `railway port list`
- Verify domain: `railway domain`

### Can't connect from agents
- Use `/dns4/` instead of `/ip4/` for Railway domains
- Check firewall rules

### High memory usage
- Reduce `--max-circuits` in Dockerfile
- Railway will auto-restart if OOM

---

## Architecture

```
Agent A (NAT) → Railway Relay → Agent B (NAT)
                 ↓
            Introduction
                 ↓
        Agent A ←→ Agent B (Direct P2P)
```

Railway provides:
- Public static IP
- Automatic HTTPS
- Auto-restart on failure
- Log aggregation
- Metrics

---

## Files

- `Dockerfile.railway` - Optimized for Railway
- `railway.json` - Railway configuration
- `RAILWAY_DEPLOY.md` - This file

---

**Next Steps:**
1. Deploy to Railway
2. Get public URL
3. Test with local agent
4. Update bootstrap peers in production
