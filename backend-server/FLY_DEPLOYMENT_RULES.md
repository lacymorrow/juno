# Fly.io Deployment Rules & Guidelines 🚀

## Overview

This document outlines the rules, best practices, and procedures for deploying and maintaining the Juno Cloud Backend on Fly.io.

## ✅ Deployment Status

**PRODUCTION DEPLOYED**: `juno-cloud-backend.fly.dev`

- **Deployed**: June 9, 2025
- **Status**: ✅ Healthy and operational
- **Region**: Atlanta (atl)
- **URL**: <https://juno-cloud-backend.fly.dev>

---

## 🔧 Pre-Deployment Rules

### 1. **MANDATORY: Test Before Deploy**

```bash
# ALWAYS run the deployment test script before any deployment
./deploy-test.sh

# Verify all tests pass with exit code 0
echo $?  # Should return 0
```

### 2. **Environment Configuration**

- **NEVER** commit actual secrets to git
- **ALWAYS** use Fly secrets for sensitive data
- **REQUIRED** environment variables:

  ```bash
  JWT_SECRET=<32-char-minimum-hex>
  HMAC_SECRET=<32-char-minimum-hex>
  NODE_ENV=production
  PORT=8080
  ```

### 3. **File Structure Requirements**

- ✅ `fly.toml` - Must exist and be valid
- ✅ `Dockerfile` - Must build successfully
- ✅ `package.json` - Must have valid start script
- ✅ `env.example` - Must exist (NOT `.env.example`)
- ✅ `src/` directory - Must contain server.js

---

## 🚀 Deployment Process

### Initial Deployment

```bash
# 1. Verify Fly CLI installation
flyctl version

# 2. Login to Fly.io
flyctl auth login

# 3. Create app (if doesn't exist)
flyctl apps create juno-cloud-backend

# 4. Create volume for database
flyctl volumes create data_volume --size 1 --region atl

# 5. Generate and set secrets
JWT_SECRET=$(openssl rand -hex 32)
HMAC_SECRET=$(openssl rand -hex 32)
flyctl secrets set JWT_SECRET=$JWT_SECRET HMAC_SECRET=$HMAC_SECRET

# 6. Deploy
flyctl deploy
```

### Subsequent Deployments

```bash
# 1. Run tests
./deploy-test.sh

# 2. Deploy (if tests pass)
flyctl deploy

# 3. Verify deployment
flyctl status
curl https://juno-cloud-backend.fly.dev/health
```

---

## 📋 Configuration Rules

### 1. **fly.toml Configuration**

- **Region**: Must match volume region (`atl`)
- **Memory**: Minimum 512MB for Node.js app
- **Port**: Internal port 8080 (matches Express server)
- **Volume**: Must be mounted to `/app/data`
- **Health checks**: Required for auto-scaling

### 2. **Dockerfile Standards**

- **Base image**: `node:20-alpine` (LTS)
- **User**: Non-root user (`juno:nodejs`)
- **Security**: File permissions 755, proper ownership
- **Health check**: Must include curl for health endpoint
- **Dependencies**: Production-only (`npm ci --only=production`)

### 3. **Environment Variables**

```bash
# Production (Fly.io)
NODE_ENV=production
PORT=8080
DB_PATH=/app/data/juno.db
LOG_LEVEL=info

# Secrets (via flyctl secrets)
JWT_SECRET=<generated-32-char-hex>
HMAC_SECRET=<generated-32-char-hex>
```

---

## 🔍 Monitoring & Maintenance

### Daily Checks

```bash
# Status check
flyctl status

# Health endpoint
curl https://juno-cloud-backend.fly.dev/health

# Recent logs
flyctl logs --no-tail | tail -20
```

### Weekly Maintenance

```bash
# Check resource usage
flyctl metrics

# Database backup (if needed)
flyctl ssh console -C "cp /app/data/juno.db /app/data/backup-$(date +%Y%m%d).db"

# Security updates
npm audit
```

### Monthly Reviews

- Review Fly.io billing and usage
- Check for security updates
- Review application logs for patterns
- Performance optimization opportunities

---

## 🚨 Troubleshooting Rules

### Health Check Failures

1. **Check logs first**: `flyctl logs`
2. **Verify endpoints**: Test `/health` manually
3. **Check database**: Ensure volume is mounted
4. **Resource limits**: Monitor memory/CPU usage

### Deployment Failures

1. **Docker build errors**: Check Dockerfile syntax
2. **Volume issues**: Ensure volume exists in correct region
3. **Secret errors**: Verify all required secrets are set
4. **Network issues**: Check security groups and ports

### Common Issues & Solutions

```bash
# Issue: Volume in wrong region
flyctl volumes create data_volume --size 1 --region atl

# Issue: Missing secrets
flyctl secrets set JWT_SECRET=$(openssl rand -hex 32)

# Issue: Health check timeout
# Check: Application actually listening on port 8080
# Check: Health endpoint returns 200 status

# Issue: Database connection errors
# Check: Volume mounted correctly
# Check: Database file permissions
```

---

## 🔒 Security Rules

### 1. **Secret Management**

- **NEVER** hardcode secrets in code
- **ALWAYS** use `flyctl secrets set`
- **ROTATE** secrets regularly (quarterly)
- **AUDIT** secret access logs

### 2. **Access Control**

- Limit Fly.io organization access
- Use 2FA on Fly.io account
- Regular access reviews
- Monitor deployment logs

### 3. **Network Security**

- HTTPS only (forced in fly.toml)
- CORS properly configured
- Rate limiting enabled
- Input validation on all endpoints

---

## 📊 Performance Rules

### Scaling Guidelines

```bash
# Scale up for high load
flyctl scale count 2  # Multiple instances
flyctl scale memory 1024  # More memory

# Scale down for cost optimization
flyctl scale count 1
flyctl scale memory 512
```

### Resource Limits

- **Memory**: 512MB minimum, 1GB recommended for production
- **CPU**: 1 shared CPU sufficient for most workloads
- **Storage**: 1GB volume minimum, monitor usage
- **Connections**: 25 concurrent connections (configured in fly.toml)

### Performance Monitoring

```bash
# Check metrics
flyctl metrics

# Monitor response times
curl -w "@curl-format.txt" https://juno-cloud-backend.fly.dev/health

# Database performance
# Monitor query times in application logs
```

---

## 🔄 Backup & Recovery

### Database Backup

```bash
# Manual backup
flyctl ssh console -C "cp /app/data/juno.db /app/data/backup-$(date +%Y%m%d).db"

# Restore from backup
flyctl ssh console -C "cp /app/data/backup-YYYYMMDD.db /app/data/juno.db"
```

### Configuration Backup

- `fly.toml` - Version controlled in git
- Secrets - Document (without values) in secure location
- Deployment scripts - Version controlled

### Disaster Recovery

1. **App recreation**: `flyctl apps create juno-cloud-backend`
2. **Volume recreation**: `flyctl volumes create data_volume --size 1`
3. **Secret restoration**: `flyctl secrets set` (from secure backup)
4. **Redeploy**: `flyctl deploy`

---

## 📝 Change Management

### Deployment Approval Process

1. **Test locally**: All tests must pass
2. **Code review**: Required for production changes
3. **Staging deployment**: Test in non-production first (if available)
4. **Production deployment**: During maintenance window
5. **Post-deployment verification**: Health checks and functionality tests

### Rollback Procedure

```bash
# Quick rollback to previous version
flyctl deploy --image registry.fly.io/juno-cloud-backend:deployment-<PREVIOUS_ID>

# Check deployment history
flyctl releases

# Rollback to specific release
flyctl releases rollback <RELEASE_ID>
```

---

## 📞 Emergency Contacts & Procedures

### Emergency Response

1. **Health check failure**: Check logs, restart if necessary
2. **Complete outage**: Check Fly.io status page, contact support
3. **Security incident**: Rotate secrets, review logs, document incident
4. **Data loss**: Restore from backup, investigate cause

### Support Resources

- **Fly.io Documentation**: <https://fly.io/docs/>
- **Fly.io Status**: <https://status.fly.io/>
- **Fly.io Community**: <https://community.fly.io/>
- **Emergency Support**: Create ticket in Fly.io dashboard

---

## 📈 Success Metrics

### Key Performance Indicators

- **Uptime**: Target 99.9%
- **Response time**: < 200ms for health checks
- **Error rate**: < 0.1%
- **Deployment success**: 100% successful deployments

### Monitoring Endpoints

- **Health**: `https://juno-cloud-backend.fly.dev/health`
- **Metrics**: `https://juno-cloud-backend.fly.dev/metrics`
- **API Status**: Test registration endpoint weekly

---

## 🏷️ Version Information

- **Document Version**: 1.0
- **Last Updated**: June 9, 2025
- **Next Review**: July 9, 2025
- **Owner**: Juno AI Development Team

---

**Remember**: When in doubt, test first, deploy carefully, and monitor actively! 🚀
