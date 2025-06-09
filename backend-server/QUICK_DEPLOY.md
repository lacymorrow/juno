# Quick Cloud Deployment Guide 🚀

This guide covers the **fastest** ways to deploy your Juno Cloud Backend to production.

## 🔥 30-Second Deployments

### 1. Railway (Easiest - Recommended)

**Time**: 5 minutes | **Cost**: $5-20/month | **Difficulty**: ⭐

```bash
# 1. Push to GitHub (if not already done)
git add .
git commit -m "Ready for deployment"
git push origin main

# 2. Visit railway.app
# 3. Connect GitHub → Select repo → Deploy
# 4. Railway auto-detects Node.js and deploys!
```

**URL**: Automatic HTTPS domain (e.g., `https://juno-backend-production.railway.app`)

### 2. Render

**Time**: 10 minutes | **Cost**: $7-25/month | **Difficulty**: ⭐⭐

```bash
# 1. Push to GitHub
git add .
git commit -m "Add render.yaml"
git push origin main

# 2. Visit render.com
# 3. Connect GitHub repo
# 4. Render uses the included render.yaml for auto-deployment
```

**Features**: Auto-deploy on git push, custom domains, SSL included

### 3. Fly.io (Global Edge)

**Time**: 15 minutes | **Cost**: $2-10/month | **Difficulty**: ⭐⭐⭐

```bash
# 1. Install Fly CLI
brew install flyctl

# 2. Login and deploy
fly auth login
cd backend-server
fly launch --generate-name

# 3. Create volume for database
fly volumes create data_volume --size 1

# 4. Set secrets
fly secrets set JWT_SECRET=$(openssl rand -hex 32)
fly secrets set HMAC_SECRET=$(openssl rand -hex 32)

# 5. Deploy
fly deploy
```

**Features**: Global CDN, excellent WebSocket support, auto-scaling

## 🛠️ Pre-Deployment Checklist

Run our deployment test script first:

```bash
./deploy-test.sh
```

This will verify:

- ✅ All required files exist
- ✅ Dependencies install correctly  
- ✅ Server starts and responds to health checks
- ✅ API endpoints work (registration, auth, metrics)
- ✅ Production readiness warnings

## 🔧 Environment Variables

For any platform, you'll need these environment variables:

### Required

```bash
NODE_ENV=production
PORT=8080
JWT_SECRET=your-super-secure-jwt-secret-32-chars-minimum
HMAC_SECRET=your-hmac-secret-32-chars-minimum
```

### Optional (with defaults)

```bash
DB_PATH=./data/juno.db
LOG_LEVEL=info
RATE_LIMIT_MAX_REQUESTS=100
ALLOWED_ORIGINS=*
```

### Generate Secure Secrets

```bash
# Generate JWT secret
openssl rand -hex 32

# Generate HMAC secret  
openssl rand -hex 32
```

## 🎯 Platform-Specific Instructions

### Railway Setup

1. **Connect GitHub**: Link your repository
2. **Auto-deploy**: Railway detects `package.json` and deploys
3. **Environment Variables**: Add in Railway dashboard
4. **Custom Domain**: Available in settings (optional)

### Render Setup  

1. **Connect GitHub**: Link your repository
2. **Auto-config**: Uses included `render.yaml` configuration
3. **Persistent Storage**: Automatically configured for database
4. **SSL**: Included with all plans

### Fly.io Setup

1. **Global Deployment**: Deploys to edge locations worldwide
2. **Volume Storage**: Persistent storage for SQLite database
3. **Secrets Management**: Encrypted environment variables
4. **Auto-scaling**: Scales based on demand

## 📊 Feature Comparison

| Feature | Railway | Render | Fly.io |
|---------|---------|--------|--------|
| **Deployment Speed** | ⚡ Instant | ⚡ Fast | ⚡ Fast |
| **Auto HTTPS** | ✅ | ✅ | ✅ |
| **Custom Domains** | ✅ | ✅ | ✅ |
| **WebSocket Support** | ✅ | ✅ | ✅⭐ |
| **Global CDN** | ❌ | ❌ | ✅ |
| **Auto-scaling** | ❌ | ❌ | ✅ |
| **Database Storage** | ✅ | ✅ | ✅ |
| **Logs & Monitoring** | ✅ | ✅ | ✅ |

## 🚀 After Deployment

### 1. Test Your Deployment

```bash
# Replace with your deployed URL
BACKEND_URL="https://your-app.railway.app"

# Test health check
curl $BACKEND_URL/health

# Test device registration
curl -X POST -H "Content-Type: application/json" \
  -d '{"device_name":"test","device_type":"desktop","platform":"macos"}' \
  $BACKEND_URL/api/register
```

### 2. Update Your Juno Client

Update your Tauri app's configuration to point to the deployed backend:

```rust
// In your cloud config
let backend_url = "wss://your-app.railway.app/ws";
let api_base = "https://your-app.railway.app/api";
```

### 3. Monitor Your Deployment

- **Health**: `https://your-app.railway.app/health`
- **Metrics**: `https://your-app.railway.app/metrics`
- **Logs**: Available in your platform's dashboard

## 🔒 Security Considerations

### Production Settings

```bash
# Use strong secrets (32+ characters)
JWT_SECRET=your-very-long-random-secret-here
HMAC_SECRET=another-very-long-random-secret

# Restrict CORS for production
ALLOWED_ORIGINS=https://your-client-domain.com

# Set production logging
LOG_LEVEL=info
NODE_ENV=production
```

### Domain & SSL

- All platforms provide automatic HTTPS
- Custom domains available on all platforms
- SSL certificates automatically managed

## 🎯 Recommended Deployment Path

### For Quick Testing

1. **Railway** - Fastest deployment, great for demos and development

### For Production Use  

1. **Render** - Best balance of features and simplicity
2. **Fly.io** - Best performance with global edge deployment

### For Enterprise

See `CLOUD_DEPLOYMENT.md` for AWS, Google Cloud, and Azure options with advanced features like:

- Load balancing
- Database clustering  
- Advanced monitoring
- Custom VPC/networking

## 🆘 Troubleshooting

### Common Issues

1. **Build Failures**: Run `./deploy-test.sh` locally first
2. **Environment Variables**: Check required variables are set
3. **Database Issues**: Ensure persistent storage is configured
4. **WebSocket Issues**: Verify platform supports WebSocket connections

### Get Help

- Platform documentation links in `CLOUD_DEPLOYMENT.md`
- Check logs in your platform's dashboard
- Test locally with `npm start` before deploying

**Ready to deploy? Pick a platform above and your Juno Cloud Backend will be live in minutes!** 🚀
