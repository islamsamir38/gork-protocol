# Deploy Gork Relay on Railway

## Prerequisites

1. Railway account (free tier works)
2. This repo pushed to GitHub

---

## Option 1: One-Click Deploy

1. Push this repo to GitHub
2. Go to [Railway](https://railway.app)
3. Click "New Project" → "Deploy from GitHub repo"
4. Select this repo
5. Railway auto-detects Dockerfile
6. Click "Deploy"

---

## Option 2: CLI Deploy

```bash
# Install Railway CLI
npm install -g @railway/cli

# Login
railway login

# Navigate to repo
cd /Users/asil/.openclaw/workspace/gork-protocol

# Initialize
railway init

# Deploy
railway up

# Get public URL
railway domain
```

---

## After Deployment

### 1. Get Relay Address

```bash
# Check logs for peer ID
railway logs | grep "Peer ID"

# Or check the public URL
railway domain
# Example: gork-relay-production.up.railway.app
```

### 2. Test Relay

```bash
# Local test
cd /Users/asil/.openclaw/workspace/gork-protocol
./target/release/gork-agent daemon \
  --bootstrap-peers /dns4/gork-relay-production.up.railway.app/tcp/4001/p2p/<PEER_ID>
```

### 3. Check Health

```bash
# Metrics endpoint
curl https://gork-relay-production.up.railway.app:9090/health

# Should return: OK
```

---

## Configuration

Railway automatically:
- Builds from `Dockerfile.railway`
- Exposes port 4001 (P2P) and 9090 (metrics)
- Provides public URL
- Restarts on failure

---

## Cost

- **Free tier:** 500 hours/month (enough for testing)
- **Hobby:** $5/month (always-on)
- **Usage:** ~512MB RAM, minimal CPU

---

## Environment Variables (Optional)

In Railway dashboard, add:

```
RUST_LOG=debug    # More verbose logging
```

---

## Troubleshooting

**Build fails:**
- Check Dockerfile.railway exists
- Verify Rust version compatibility

**Can't connect:**
- Use `/dns4/` not `/ip4/` for Railway
- Check port 4001 is exposed

**High costs:**
- Reduce max-circuits in Dockerfile
- Use free tier for testing

---

## Next Steps

1. ✅ Deploy to Railway
2. ✅ Get public URL
3. ✅ Test with local agent
4. ✅ Update bootstrap peers in production agents
5. ✅ Monitor via Railway dashboard

---

**Files created:**
- `Dockerfile.railway` - Railway-optimized Docker image
- `railway.json` - Railway configuration
- `DEPLOY_RAILWAY.md` - This guide
