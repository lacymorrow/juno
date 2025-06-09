# Juno Cloud Backend - Unraid Deployment Guide

This guide will help you deploy the Juno Cloud Backend on your Unraid server using Docker.

## 🚀 Quick Start

### Option 1: Docker Compose (Recommended)

1. **Create the application directory:**
   ```bash
   mkdir -p /mnt/user/appdata/juno-backend
   cd /mnt/user/appdata/juno-backend
   ```

2. **Copy your files:**
   ```bash
   # Copy the entire backend-server directory to this location
   # You can use the Unraid file manager or SCP
   ```

3. **Create your environment file:**
   ```bash
   cp .env.example .env
   nano .env
   ```
   
   **Required environment variables:**
   ```env
   # Generate strong secrets (32+ characters each)
   JWT_SECRET=your-super-secure-jwt-secret-key-change-this-to-something-very-long-and-random
   HMAC_SECRET=your-super-secure-hmac-secret-key-change-this-to-something-very-long-and-random
   
   # Server configuration
   NODE_ENV=production
   PORT=8080
   HOST=0.0.0.0
   LOG_LEVEL=info
   
   # CORS (update for security)
   CORS_ORIGIN=*
   ```

4. **Start the service:**
   ```bash
   docker-compose up -d
   ```

### Option 2: Unraid Community Applications

1. Go to **Apps** in your Unraid web interface
2. Search for "Juno" (if/when available in Community Apps)
3. Install and configure

### Option 3: Manual Docker Container

```bash
docker run -d \
  --name juno-cloud-backend \
  --restart unless-stopped \
  -p 8080:8080 \
  -v /mnt/user/appdata/juno-backend/data:/app/data \
  -v /mnt/user/appdata/juno-backend/logs:/app/logs \
  -e NODE_ENV=production \
  -e JWT_SECRET="your-jwt-secret" \
  -e HMAC_SECRET="your-hmac-secret" \
  -e PORT=8080 \
  -e LOG_LEVEL=info \
  juno-backend:latest
```

## 📁 Directory Structure

```
/mnt/user/appdata/juno-backend/
├── docker-compose.yml
├── .env
├── src/
├── package.json
├── data/              # Database storage (persistent)
├── logs/              # Application logs (persistent)
└── README.md
```

## 🔧 Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `JWT_SECRET` | ✅ | - | Secret for JWT token signing (32+ chars) |
| `HMAC_SECRET` | ✅ | - | Secret for HMAC authentication (32+ chars) |
| `PORT` | ❌ | 8080 | Server port |
| `LOG_LEVEL` | ❌ | info | Logging level (error, warn, info, debug) |
| `CORS_ORIGIN` | ❌ | * | CORS allowed origins |
| `DB_PATH` | ❌ | ./data/juno.db | Database file path |

### Port Configuration

- **Internal Port:** 8080 (WebSocket + HTTP API)
- **External Port:** 8080 (or any available port on your Unraid)
- **Protocol:** TCP

### Volume Mounts

- **Database:** `/mnt/user/appdata/juno-backend/data` → `/app/data`
- **Logs:** `/mnt/user/appdata/juno-backend/logs` → `/app/logs`
- **Config:** `/mnt/user/appdata/juno-backend/.env` → `/app/.env`

## 🛡️ Security Configuration

### Generate Secure Secrets

```bash
# Generate JWT Secret
openssl rand -hex 32

# Generate HMAC Secret  
openssl rand -hex 32
```

### Network Security

1. **Internal Access Only:**
   ```yaml
   # In docker-compose.yml, comment out ports section
   # ports:
   #   - "8080:8080"
   ```

2. **Custom Network:**
   ```yaml
   networks:
     juno-internal:
       driver: bridge
       ipam:
         config:
           - subnet: 172.20.0.0/16
   ```

3. **Reverse Proxy (nginx/Cloudflare):**
   - Use nginx proxy manager in Unraid
   - Configure SSL/TLS termination
   - Set up authentication if needed

## 📊 Monitoring & Logs

### Check Container Status
```bash
docker-compose ps
docker logs juno-cloud-backend
```

### Health Check Endpoint
```bash
curl http://your-unraid-ip:8080/health
```

### View Application Logs
```bash
# Follow logs in real-time
docker-compose logs -f juno-backend

# View logs in Unraid
tail -f /mnt/user/appdata/juno-backend/logs/server.log
```

## 🔄 Maintenance

### Backup Database
```bash
# Create backup
cp /mnt/user/appdata/juno-backend/data/juno.db /mnt/user/appdata/juno-backend/backups/juno-$(date +%Y%m%d).db

# Automated backup (add to User Scripts)
#!/bin/bash
BACKUP_DIR="/mnt/user/appdata/juno-backend/backups"
mkdir -p $BACKUP_DIR
cp /mnt/user/appdata/juno-backend/data/juno.db "$BACKUP_DIR/juno-$(date +%Y%m%d-%H%M).db"
find $BACKUP_DIR -name "juno-*.db" -mtime +7 -delete
```

### Update Container
```bash
cd /mnt/user/appdata/juno-backend
docker-compose pull
docker-compose up -d
```

### Restart Service
```bash
docker-compose restart juno-backend
```

## 🧪 Testing Connection

### Test from Tauri App

1. Update your Tauri app configuration:
   ```rust
   // In your cloud config
   server_url: "ws://your-unraid-ip:8080"
   ```

2. Test WebSocket connection:
   ```bash
   # Install wscat if not available
   npm install -g wscat
   
   # Test connection
   wscat -c ws://your-unraid-ip:8080
   ```

### Register Test Device

```bash
curl -X POST http://your-unraid-ip:8080/api/register \
  -H "Content-Type: application/json" \
  -d '{
    "device_name": "Test Device",
    "device_type": "desktop"
  }'
```

## 🚨 Troubleshooting

### Common Issues

1. **Container won't start:**
   ```bash
   docker-compose logs juno-backend
   # Check for environment variable errors
   ```

2. **Port conflicts:**
   ```bash
   # Check if port 8080 is in use
   netstat -tulpn | grep 8080
   
   # Use different port in docker-compose.yml
   ports:
     - "8081:8080"
   ```

3. **Permission errors:**
   ```bash
   # Fix file permissions
   chown -R nobody:users /mnt/user/appdata/juno-backend
   chmod -R 755 /mnt/user/appdata/juno-backend
   ```

4. **Database locked:**
   ```bash
   # Stop container and check for zombie processes
   docker-compose down
   ps aux | grep node
   ```

### Log Analysis

```bash
# Check for authentication errors
grep "auth" /mnt/user/appdata/juno-backend/logs/server.log

# Check for connection issues
grep "WebSocket\|connection" /mnt/user/appdata/juno-backend/logs/server.log

# Check for rate limiting
grep "rate limit" /mnt/user/appdata/juno-backend/logs/server.log
```

## 📈 Performance Tuning

### Resource Limits

```yaml
# In docker-compose.yml
deploy:
  resources:
    limits:
      memory: 1G      # Increase for high load
      cpus: '1.0'     # Increase for more clients
```

### Database Optimization

```bash
# Enable WAL mode for better concurrent access
sqlite3 /mnt/user/appdata/juno-backend/data/juno.db "PRAGMA journal_mode=WAL;"
```

## 🔐 Advanced Security

### Enable HTTPS

1. Use nginx proxy manager
2. Set up Let's Encrypt certificates
3. Configure reverse proxy to backend

### Firewall Rules

```bash
# Allow only specific IPs (adjust as needed)
iptables -A INPUT -p tcp --dport 8080 -s 192.168.1.0/24 -j ACCEPT
iptables -A INPUT -p tcp --dport 8080 -j DROP
```

## 📞 Support

- Check logs first: `/mnt/user/appdata/juno-backend/logs/`
- Health endpoint: `http://your-unraid-ip:8080/health`
- Database backup before making changes
- Test with minimal configuration first

---

## 🎯 Next Steps

1. **Set up monitoring** with Unraid system stats
2. **Configure automated backups** using User Scripts
3. **Set up reverse proxy** for HTTPS access
4. **Integrate with your Tauri client** for testing
5. **Set up alerting** for service health 
