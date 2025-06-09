# Use Node.js LTS version for stability
FROM node:20-alpine

# Set NODE_ENV
ENV NODE_ENV=production

# Create app directory
WORKDIR /app

# Create non-root user for security
RUN addgroup -g 1001 -S nodejs && \
    adduser -S juno -u 1001

# Copy package files
COPY package*.json ./

# Install dependencies (production only)
RUN npm ci --only=production && \
    npm cache clean --force

# Copy application source
COPY src/ ./src/
COPY env.example ./env.example

# Create necessary directories with proper permissions
RUN mkdir -p data logs && \
    chown -R juno:nodejs /app && \
    chmod -R 755 /app

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:${PORT:-8080}/health || exit 1

# Install curl for health check
RUN apk add --no-cache curl

# Switch to non-root user
USER juno

# Expose port
EXPOSE 8080

# Start the application
CMD ["npm", "start"]
