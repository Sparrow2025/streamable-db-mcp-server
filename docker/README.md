# Docker Deployment Guide

This directory contains Docker configuration and scripts for deploying the MySQL MCP Server.

## Quick Start

### 1. Build and Start (Production)

```bash
# Build and start in production mode
./docker/start.sh

# Or manually:
docker-compose up -d
```

### 2. Build and Start (Development)

```bash
# Build and start in development mode
./docker/start.sh --dev

# Or manually:
docker-compose -f docker-compose.dev.yml up -d
```

### 3. Test the Deployment

```bash
# Run comprehensive tests
./docker/test.sh

# Or test manually:
curl http://localhost:8080/health
```

## Available Scripts

### `build.sh`
Builds both production and development Docker images.

```bash
./docker/build.sh
```

### `start.sh`
Starts the services in production or development mode.

```bash
./docker/start.sh [--prod|--dev]
```

Options:
- `--prod`, `--production`: Start in production mode (default)
- `--dev`, `--development`: Start in development mode with hot reload

### `test.sh`
Runs comprehensive tests against the running Docker deployment.

```bash
./docker/test.sh
```

Tests include:
- Health check endpoint
- MCP protocol initialization
- Tool listing
- Database connection
- Query execution
- Database listing

### `cleanup.sh`
Cleans up Docker resources.

```bash
./docker/cleanup.sh [--full]
```

Options:
- `--full`: Remove images, volumes, and networks (complete cleanup)

## Docker Compose Files

### `docker-compose.yml` (Production)
- Optimized production build
- Minimal container size
- Production logging level
- Health checks enabled

### `docker-compose.dev.yml` (Development)
- Volume mounts for hot reload
- Debug logging enabled
- Cargo cache persistence
- Development-friendly configuration

## Container Architecture

### MySQL MCP Server Container
- **Base Image**: `debian:bookworm-slim`
- **Runtime User**: `mcp` (non-root)
- **Port**: 8080
- **Health Check**: `/health` endpoint
- **Environment Variables**:
  - `RUST_LOG`: Logging level
  - `PORT`: Server port
  - `DB_HOST`, `DB_PORT`, `DB_USERNAME`, `DB_PASSWORD`, `DB_DATABASE`: Database connection

### MySQL Database Container
- **Base Image**: `mysql:8.0`
- **Port**: 3306
- **Default Database**: `mcp_test`
- **Default User**: `mcp_user` / `mcp_password`
- **Initialization**: Runs `docker/mysql/init.sql` on first start

## Configuration

### Environment Variables

The MCP server supports configuration via environment variables:

```bash
# Database connection
DB_HOST=mysql-db
DB_PORT=3306
DB_USERNAME=mcp_user
DB_PASSWORD=mcp_password
DB_DATABASE=mcp_test

# Server configuration
PORT=8080
RUST_LOG=info
```

### Volume Mounts

#### Production
- `mysql_data`: MySQL data persistence

#### Development
- `.:/app`: Source code hot reload
- `cargo_cache`: Cargo registry cache
- `target_cache`: Rust build cache
- `mysql_dev_data`: MySQL data persistence

## Networking

All services run on the `mcp-network` bridge network:

- **mysql-mcp-server**: Accessible on `localhost:8080`
- **mysql-db**: Accessible on `localhost:3306` and internally as `mysql-db:3306`

## Health Checks

### MCP Server Health Check
- **Endpoint**: `GET /health`
- **Interval**: 30 seconds
- **Timeout**: 10 seconds
- **Retries**: 3
- **Start Period**: 40 seconds

Response format:
```json
{
  "status": "healthy",
  "timestamp": "2024-01-01T12:00:00Z",
  "service": "mysql-mcp-server",
  "version": "0.1.0"
}
```

### MySQL Health Check
- **Command**: `mysqladmin ping`
- **Interval**: 10 seconds
- **Timeout**: 5 seconds
- **Retries**: 5
- **Start Period**: 30 seconds

## Troubleshooting

### Common Issues

#### 1. Port Already in Use
```bash
# Check what's using port 8080
lsof -i :8080

# Or use different port
PORT=8081 docker-compose up -d
```

#### 2. Database Connection Failed
```bash
# Check MySQL container logs
docker-compose logs mysql-db

# Test MySQL connection
docker exec -it mysql-mcp-db mysql -u mcp_user -pmcp_password mcp_test
```

#### 3. Build Failures
```bash
# Clean build cache
docker system prune -f
docker builder prune -f

# Rebuild from scratch
./docker/cleanup.sh --full
./docker/build.sh
```

### Viewing Logs

```bash
# All services
docker-compose logs -f

# Specific service
docker-compose logs -f mysql-mcp-server
docker-compose logs -f mysql-db

# Last N lines
docker-compose logs --tail=50 mysql-mcp-server
```

### Debugging

#### Enter Container Shell
```bash
# MCP Server container
docker exec -it mysql-mcp-server /bin/bash

# MySQL container
docker exec -it mysql-mcp-db /bin/bash
```

#### Check Container Status
```bash
# Container information
docker inspect mysql-mcp-server

# Resource usage
docker stats mysql-mcp-server mysql-mcp-db
```

## Security Considerations

### Container Security
- Runs as non-root user (`mcp`)
- Minimal base image (Debian Slim)
- No unnecessary packages installed
- Health checks for monitoring

### Database Security
- Dedicated database user with limited permissions
- Password-protected access
- Network isolation via Docker networks
- Data persistence in named volumes

### Configuration Security
- Sensitive data via environment variables
- No hardcoded credentials in images
- `.dockerignore` excludes sensitive files

## Performance Optimization

### Production Optimizations
- Multi-stage build for smaller images
- Release build with optimizations
- Minimal runtime dependencies
- Efficient layer caching

### Development Optimizations
- Cargo cache persistence
- Volume mounts for hot reload
- Debug symbols for debugging
- Separate development database

## Monitoring

### Health Endpoints
- **MCP Server**: `http://localhost:8080/health`
- **MCP Protocol**: `http://localhost:8080/mcp`

### Metrics Collection
The containers expose standard Docker metrics that can be collected by monitoring systems like Prometheus.

### Log Aggregation
Logs are written to stdout/stderr and can be collected by log aggregation systems like ELK stack or Fluentd.

## Production Deployment

### Recommended Setup
1. Use production compose file
2. Set up proper monitoring
3. Configure log rotation
4. Set up backup for MySQL data
5. Use secrets management for credentials
6. Set up reverse proxy (nginx/traefik)
7. Configure SSL/TLS termination

### Scaling Considerations
- The current setup is single-instance
- For scaling, consider:
  - Load balancer for multiple MCP server instances
  - MySQL clustering or read replicas
  - Shared configuration management
  - Container orchestration (Kubernetes)

## Integration with CI/CD

### Example GitHub Actions
```yaml
- name: Build and Test Docker
  run: |
    ./docker/build.sh
    ./docker/start.sh
    sleep 30
    ./docker/test.sh
    ./docker/cleanup.sh
```

### Example GitLab CI
```yaml
docker-test:
  script:
    - ./docker/build.sh
    - ./docker/start.sh
    - ./docker/test.sh
  after_script:
    - ./docker/cleanup.sh --full
```