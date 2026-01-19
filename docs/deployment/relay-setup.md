# Lucidity Relay Setup Guide

This guide covers how to deploy and configure the Lucidity Relay server.

## Prerequisites

- Docker installed locally
- Access to a container registry (GHCR, Docker Hub, or private)
- Cloud platform account (Render, Fly.io, or similar)
- Domain name for TLS (optional but recommended)

---

## Deployment Options

### 1. Render (Recommended)

Render is ideal for the Lucidity Relay because it handles TLS termination and provides a stable public URL.

#### Quick Setup

1. Go to [Render Dashboard](https://dashboard.render.com)
2. Click "New +" → "Web Service"
3. Connect your GitHub repository
4. Configure:
   - **Name**: `lucidity-relay`
   - **Root Directory**: `lucidity-relay`
   - **Runtime**: Docker
   - **Dockerfile Path**: `Dockerfile`
   - **Instance Type**: Starter ($7/mo) or higher

#### Environment Variables

| Variable | Value | Notes |
|----------|-------|-------|
| `LUCIDITY_RELAY_LISTEN` | `0.0.0.0:10000` | Render uses port 10000 |
| `LUCIDITY_RELAY_DESKTOP_SECRET` | `<random-string>` | Generate with `openssl rand -hex 32` |
| `LUCIDITY_RELAY_REQUIRE_TLS` | `false` | Render handles TLS termination |
| `RUST_LOG` | `lucidity_relay=info` | Logging level |

#### Verify Deployment

```bash
curl https://lucidity-relay.onrender.com/healthz
# Should return: OK
```

---

### 2. Fly.io

#### Install flyctl

```bash
# macOS/Linux
curl -L https://fly.io/install.sh | sh

# Windows
powershell -Command "iwr https://fly.io/install.ps1 -useb | iex"
```

#### Create and Deploy

```bash
cd lucidity-relay
fly launch --name lucidity-relay --no-deploy

# Set secrets
fly secrets set LUCIDITY_RELAY_DESKTOP_SECRET=$(openssl rand -hex 32)

# Deploy
fly deploy
```

#### Verify

```bash
curl https://lucidity-relay.fly.dev/healthz
```

---

### 3. Docker / Self-Hosting

You can run the relay anywhere that supports Docker.

#### Build

```bash
cd lucidity-relay
docker build -t lucidity-relay:latest -f Dockerfile ..
```

#### Run

```bash
docker run -p 9090:9090 \
  -e LUCIDITY_RELAY_LISTEN=0.0.0.0:9090 \
  -e LUCIDITY_RELAY_DESKTOP_SECRET=your_secret \
  -e RUST_LOG=lucidity_relay=info \
  lucidity-relay:latest
```

#### With docker-compose

```yaml
version: '3.8'
services:
  relay:
    build:
      context: ..
      dockerfile: lucidity-relay/Dockerfile
    ports:
      - "9090:9090"
    environment:
      - LUCIDITY_RELAY_LISTEN=0.0.0.0:9090
      - LUCIDITY_RELAY_DESKTOP_SECRET=${RELAY_SECRET}
      - RUST_LOG=lucidity_relay=info
    restart: unless-stopped
```

---

## Configuration Reference

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `LUCIDITY_RELAY_LISTEN` | Bind address | `127.0.0.1:9090` | `0.0.0.0:9090` |
| `LUCIDITY_RELAY_DESKTOP_SECRET` | HMAC secret for desktop auth | (required) | `a-long-random-string` |
| `LUCIDITY_RELAY_JWT_SECRET` | Secret for Mobile JWTs | (optional) | `another-random-string` |
| `LUCIDITY_RELAY_REQUIRE_TLS` | Enforce WSS headers | `false` | `true` |
| `LUCIDITY_RELAY_NO_AUTH` | Disable all auth (DEV ONLY) | `false` | `true` |
| `RUST_LOG` | Logging level | `info` | `lucidity_relay=debug` |

---

## TLS Configuration

### Behind Reverse Proxy (Recommended)

When running behind Render, Fly.io, Cloudflare, or nginx:
- The platform handles TLS termination
- Set `LUCIDITY_RELAY_REQUIRE_TLS=false`
- Clients still use `wss://` URLs

### Nginx Reverse Proxy

```nginx
server {
    listen 443 ssl http2;
    server_name relay.example.com;
    
    ssl_certificate /etc/letsencrypt/live/relay.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/relay.example.com/privkey.pem;
    
    location / {
        proxy_pass http://127.0.0.1:9090;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
    }
}
```

---

## Monitoring

### Health Check Endpoint

```
GET /healthz → 200 OK
```

Used by cloud platforms for health checks and load balancer routing.

### Logging Levels

```bash
# Minimal (production)
RUST_LOG=lucidity_relay=warn

# Normal operation
RUST_LOG=lucidity_relay=info

# Debugging
RUST_LOG=lucidity_relay=debug,warp=info

# Full trace
RUST_LOG=lucidity_relay=trace
```

### Recommended Alerts

1. Health check failures (5+ consecutive)
2. Error rate > 1% of requests
3. Memory usage > 500MB
4. Connection count > 5000

---

## Scaling

### Capacity (Single Instance)

- 1000+ concurrent desktop connections
- 10,000+ concurrent mobile connections
- ~30MB base memory
- ~2KB per connection

### Horizontal Scaling

The relay is stateless for persistence, but WebSocket sessions are in-memory:
- Use sticky sessions if load balancing
- Or run single instance with larger capacity
- Redis-backed sessions planned for future

---

## Troubleshooting

### Relay won't start

```bash
docker logs lucidity-relay
```

Common issues:
- Port already in use
- Missing `LUCIDITY_RELAY_DESKTOP_SECRET`
- Permission errors

### Connections failing

1. Verify `/healthz` returns 200
2. Check TLS configuration matches platform
3. Review firewall rules (port 443 or 9090)
4. Check relay logs for errors

### High latency

1. Deploy closer to users
2. Monitor CPU/memory
3. Check network between mobile → relay → desktop

---

## Security Checklist

- [ ] Strong `LUCIDITY_RELAY_DESKTOP_SECRET` set (32+ bytes)
- [ ] TLS enabled (via platform or nginx)
- [ ] `LUCIDITY_RELAY_NO_AUTH` is NOT set in production
- [ ] Logs configured (not exposing tokens)
- [ ] Health monitoring enabled
- [ ] Regular updates applied
