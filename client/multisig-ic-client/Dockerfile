# Use Node.js LTS version
FROM node:18-alpine

# Set working directory
WORKDIR /app

# Create app user for security
RUN addgroup -g 1001 -S nodejs
RUN adduser -S expressapp -u 1001

# Install dumb-init for proper signal handling
RUN apk add --no-cache dumb-init

# Copy package files first (for better caching)
COPY package*.json ./

# Install dependencies
RUN npm ci --only=production && npm cache clean --force

# Create directories for logs and identity files
RUN mkdir -p /app/logs /app/identity

# Copy application code
COPY . .

# Set ownership and permissions
RUN chown -R expressapp:nodejs /app
RUN chmod -R 755 /app

# Create volume for persistent data
VOLUME ["/app/logs", "/app/identity"]

# Switch to non-root user
USER expressapp

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD node healthcheck.js || exit 1

# Start the application with dumb-init
ENTRYPOINT ["dumb-init", "--"]
CMD ["npm", "start"]