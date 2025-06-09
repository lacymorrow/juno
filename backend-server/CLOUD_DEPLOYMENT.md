# Cloud Deployment Guide

This guide covers deploying the Juno Cloud Backend to major cloud providers. The application is a Node.js WebSocket server with SQLite database, designed for high availability and scalability.

## 🚀 Quick Deployment Options

| Provider | Best For | Complexity | Cost | Setup Time |
|----------|----------|------------|------|------------|
| **Railway** | Simplest deployment | ⭐ | $ | 5 minutes |
| **Render** | Good balance | ⭐⭐ | $ | 10 minutes |
| **Digital Ocean** | Droplets + Apps | ⭐⭐ | $$ | 15 minutes |
| **Fly.io** | Global edge deployment | ⭐⭐⭐ | $$ | 20 minutes |
| **AWS** | Enterprise/scalability | ⭐⭐⭐⭐ | $$$ | 30 minutes |
| **Google Cloud** | Enterprise features | ⭐⭐⭐⭐ | $$$ | 30 minutes |
| **Azure** | Microsoft ecosystem | ⭐⭐⭐⭐ | $$$ | 30 minutes |

---

## 🎯 Railway (Simplest - Recommended for Quick Start)

Railway provides one-click deployment from GitHub with automatic HTTPS.

### 1. Setup Repository
```bash
# Push to GitHub if not already done
git add .
git commit -m "Add cloud deployment configs"
git push origin main
```

### 2. Deploy to Railway
1. Visit [railway.app](https://railway.app)
2. Connect your GitHub account
3. Select your repository
4. Choose `backend-server` as the source directory
5. Railway auto-detects Node.js and deploys

### 3. Configure Environment Variables
In Railway dashboard, add these environment variables:
```bash
NODE_ENV=production
PORT=8080
JWT_SECRET=your-super-secure-jwt-secret-here
HMAC_SECRET=your-hmac-secret-for-device-auth
DB_PATH=./data/juno.db
LOG_LEVEL=info
RATE_LIMIT_MAX_REQUESTS=100
ALLOWED_ORIGINS=https://your-client-domain.com
```

### 4. Add Persistent Storage
```bash
# Railway automatically provides persistent volumes
# Database will be saved to ./data/juno.db
```

**Cost**: ~$5-20/month depending on usage  
**URL**: Automatic HTTPS domain provided (e.g., `https://your-app.railway.app`)

---

## 🌊 Render

Great for production apps with good developer experience.

### 1. Create `render.yaml`
```yaml
services:
  - type: web
    name: juno-cloud-backend
    env: node
    plan: starter
    buildCommand: npm install
    startCommand: npm start
    envVars:
      - key: NODE_ENV
        value: production
      - key: PORT
        value: 8080
      - key: JWT_SECRET
        generateValue: true
      - key: HMAC_SECRET
        generateValue: true
      - key: DB_PATH
        value: /opt/render/project/data/juno.db
      - key: LOG_LEVEL
        value: info
      - key: RATE_LIMIT_MAX_REQUESTS
        value: 100
    disk:
      name: data
      mountPath: /opt/render/project/data
      sizeGB: 1
```

### 2. Deploy
1. Visit [render.com](https://render.com)
2. Connect GitHub repository
3. Render auto-deploys using `render.yaml`

**Cost**: $7-25/month for starter plans  
**Features**: Auto-deploy, custom domains, SSL included

---

## 🌊 Digital Ocean

Flexible deployment options with droplets or App Platform.

### Option A: App Platform (Managed)

Create `.do/app.yaml`:
```yaml
name: juno-cloud-backend
services:
- name: api
  source_dir: /
  github:
    repo: your-username/your-repo
    branch: main
  run_command: npm start
  environment_slug: node-js
  instance_count: 1
  instance_size_slug: basic-xxs
  http_port: 8080
  env:
  - key: NODE_ENV
    value: production
  - key: JWT_SECRET
    value: your-jwt-secret
  - key: HMAC_SECRET
    value: your-hmac-secret
  - key: DB_PATH
    value: /data/juno.db
```

### Option B: Droplet (VPS)

```bash
# Create droplet with Docker
doctl compute droplet create juno-backend \
  --image docker-20-04 \
  --size s-1vcpu-1gb \
  --region nyc1

# SSH and deploy
ssh root@droplet-ip
git clone https://github.com/your-username/your-repo
cd your-repo/backend-server
docker-compose up -d
```

**Cost**: $6-12/month for basic droplets

---

## ✈️ Fly.io

Global edge deployment with excellent WebSocket support.

### 1. Install Fly CLI
```bash
# macOS
brew install flyctl

# Login
fly auth login
```

### 2. Initialize Fly App
```bash
cd backend-server
fly launch --no-deploy

# Edit fly.toml as needed
```

### 3. Create `fly.toml`
```toml
app = "juno-cloud-backend"
primary_region = "iad"
kill_signal = "SIGINT"
kill_timeout = "5s"

[experimental]
  auto_rollback = true

[build]

[env]
  NODE_ENV = "production"
  PORT = "8080"
  LOG_LEVEL = "info"

[http_service]
  internal_port = 8080
  force_https = true
  auto_stop_machines = true
  auto_start_machines = true
  min_machines_running = 1
  processes = ["app"]

[[vm]]
  cpu_kind = "shared"
  cpus = 1
  memory_mb = 512

[mounts]
  source = "data_volume"
  destination = "/app/data"
```

### 4. Create Volume and Deploy
```bash
# Create persistent volume for database
fly volumes create data_volume --size 1

# Set secrets
fly secrets set JWT_SECRET=your-jwt-secret
fly secrets set HMAC_SECRET=your-hmac-secret

# Deploy
fly deploy
```

**Cost**: ~$2-10/month for small apps  
**Features**: Global CDN, auto-scaling, excellent WebSocket support

---

## ☁️ AWS Deployment

Multiple AWS deployment options for different needs.

### Option A: AWS App Runner (Simplest)

Create `apprunner.yaml`:
```yaml
version: 1.0
runtime: nodejs16
build:
  commands:
    build:
      - npm install
run:
  runtime-version: 16
  command: npm start
  network:
    port: 8080
    env: PORT
  env:
    - name: NODE_ENV
      value: production
    - name: LOG_LEVEL
      value: info
```

Deploy via AWS Console:
1. AWS App Runner → Create Service
2. Connect GitHub repository
3. Configure environment variables
4. Deploy

### Option B: ECS with Fargate

Create `aws-task-definition.json`:
```json
{
  "family": "juno-cloud-backend",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "256",
  "memory": "512",
  "executionRoleArn": "arn:aws:iam::ACCOUNT:role/ecsTaskExecutionRole",
  "containerDefinitions": [
    {
      "name": "juno-backend",
      "image": "your-account.dkr.ecr.region.amazonaws.com/juno-backend:latest",
      "portMappings": [
        {
          "containerPort": 8080,
          "protocol": "tcp"
        }
      ],
      "environment": [
        {"name": "NODE_ENV", "value": "production"},
        {"name": "PORT", "value": "8080"}
      ],
      "secrets": [
        {
          "name": "JWT_SECRET",
          "valueFrom": "arn:aws:secretsmanager:region:account:secret:jwt-secret"
        }
      ],
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/juno-backend",
          "awslogs-region": "us-east-1",
          "awslogs-stream-prefix": "ecs"
        }
      }
    }
  ]
}
```

### Option C: EC2 with Docker

```bash
# Create EC2 instance
aws ec2 run-instances \
  --image-id ami-0abcdef1234567890 \
  --instance-type t3.micro \
  --key-name your-key-pair \
  --security-group-ids sg-12345678 \
  --subnet-id subnet-12345678

# SSH and deploy
ssh -i your-key.pem ec2-user@instance-ip
sudo yum update -y
sudo yum install -y docker
sudo service docker start
sudo usermod -a -G docker ec2-user

# Deploy application
git clone https://github.com/your-username/your-repo
cd your-repo/backend-server
docker-compose up -d
```

**Cost**: $8-50/month depending on instance size

---

## 🌐 Google Cloud Platform

### Option A: Cloud Run (Serverless)

Create `cloudbuild.yaml`:
```yaml
steps:
- name: 'gcr.io/cloud-builders/docker'
  args: ['build', '-t', 'gcr.io/$PROJECT_ID/juno-backend', './backend-server']
- name: 'gcr.io/cloud-builders/docker'
  args: ['push', 'gcr.io/$PROJECT_ID/juno-backend']
- name: 'gcr.io/cloud-builders/gcloud'
  args:
  - 'run'
  - 'deploy'
  - 'juno-backend'
  - '--image=gcr.io/$PROJECT_ID/juno-backend'
  - '--region=us-central1'
  - '--platform=managed'
  - '--allow-unauthenticated'
```

Deploy:
```bash
# Enable APIs
gcloud services enable run.googleapis.com
gcloud services enable cloudbuild.googleapis.com

# Deploy
gcloud builds submit --config cloudbuild.yaml
```

### Option B: Compute Engine

```bash
# Create VM
gcloud compute instances create juno-backend \
  --zone=us-central1-a \
  --machine-type=e2-micro \
  --image-family=ubuntu-2004-lts \
  --image-project=ubuntu-os-cloud

# SSH and deploy
gcloud compute ssh juno-backend --zone=us-central1-a
```

**Cost**: $5-30/month for small instances

---

## 🔵 Microsoft Azure

### Option A: Container Instances

```bash
# Create resource group
az group create --name juno-rg --location eastus

# Deploy container
az container create \
  --resource-group juno-rg \
  --name juno-backend \
  --image your-registry/juno-backend:latest \
  --dns-name-label juno-backend-unique \
  --ports 8080 \
  --environment-variables \
    NODE_ENV=production \
    PORT=8080 \
  --secure-environment-variables \
    JWT_SECRET=your-jwt-secret \
    HMAC_SECRET=your-hmac-secret
```

### Option B: App Service

```bash
# Create App Service plan
az appservice plan create \
  --name juno-plan \
  --resource-group juno-rg \
  --sku B1 \
  --is-linux

# Create web app
az webapp create \
  --resource-group juno-rg \
  --plan juno-plan \
  --name juno-backend-unique \
  --deployment-container-image-name your-registry/juno-backend:latest
```

**Cost**: $10-50/month depending on plan

---

## 🛠️ Production Configuration

### Environment Variables for All Platforms
```bash
# Required
NODE_ENV=production
PORT=8080
JWT_SECRET=your-super-secure-jwt-secret-minimum-32-characters
HMAC_SECRET=your-hmac-secret-for-device-authentication-32-chars

# Database
DB_PATH=./data/juno.db

# Security
RATE_LIMIT_MAX_REQUESTS=100
RATE_LIMIT_WINDOW_MS=900000
ALLOWED_ORIGINS=https://your-client-domain.com,https://app.yourdomain.com

# Logging
LOG_LEVEL=info
LOG_FILE=./logs/server.log

# Optional
REDIS_URL=redis://your-redis-instance (for distributed rate limiting)
```

### SSL/HTTPS Configuration

Most cloud providers include SSL automatically. For custom domains:

```nginx
# Nginx reverse proxy config
server {
    listen 443 ssl;
    server_name api.yourdomain.com;
    
    ssl_certificate /path/to/certificate.pem;
    ssl_certificate_key /path/to/private.key;
    
    location / {
        proxy_pass http://localhost:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
    }
}
```

---

## 📊 Performance & Scaling

### Database Considerations

For high-traffic deployments, consider upgrading from SQLite:

#### PostgreSQL Option
```javascript
// Update Database.js for PostgreSQL
const { Pool } = require('pg');

class Database {
    constructor() {
        this.pool = new Pool({
            connectionString: process.env.DATABASE_URL,
            ssl: process.env.NODE_ENV === 'production'
        });
    }
    // ... rest of implementation
}
```

#### Redis for Rate Limiting
```javascript
// Update rateLimiter.js for Redis
const redis = require('redis');
const client = redis.createClient(process.env.REDIS_URL);

const rateLimiter = rateLimit({
    store: new RedisStore({
        client: client,
        prefix: 'juno:rate-limit:'
    }),
    // ... rest of config
});
```

### Monitoring & Alerts

Add monitoring for production deployments:

```javascript
// Add to server.js
const prometheus = require('prom-client');

// Create metrics
const httpRequestsTotal = new prometheus.Counter({
    name: 'http_requests_total',
    help: 'Total number of HTTP requests',
    labelNames: ['method', 'route', 'status']
});

// Metrics endpoint
app.get('/metrics', async (req, res) => {
    res.set('Content-Type', prometheus.register.contentType);
    res.end(await prometheus.register.metrics());
});
```

---

## 🎯 Deployment Recommendations

### For Development/Testing
- **Railway** or **Render** - Simple, fast deployment
- Cost: $5-15/month
- Perfect for prototyping and small teams

### For Production
- **AWS ECS/Fargate** or **Google Cloud Run** - Enterprise features
- **Fly.io** - Global edge deployment with excellent WebSocket support
- Cost: $20-100/month depending on scale

### For High-Scale Production
- **AWS** with ECS, RDS PostgreSQL, ElastiCache Redis
- **Google Cloud** with Cloud Run, Cloud SQL, Memorystore
- Cost: $100+/month with enterprise features

Choose based on your needs:
- **Simplicity**: Railway, Render
- **Performance**: Fly.io, Digital Ocean
- **Enterprise**: AWS, Google Cloud, Azure
- **Cost**: Railway, Digital Ocean droplets

All options support the WebSocket server and provide the scalability needed for production Juno deployments. 
