# MySQL MCP Server - Multi-Environment Docker Setup

This directory contains Docker configurations for running the MySQL MCP Server with multiple database environments.

## Overview

The multi-environment setup allows you to:
- Connect to multiple database environments simultaneously (dev, staging, production)
- Execute queries against specific environments or across multiple environments
- Compare data and schemas between environments
- Manage environment-specific configurations and credentials

## Available Configurations

### Docker Compose Files

| File | Description | Use Case |
|------|-------------|----------|
| `docker-compose.yml` | Single environment (legacy) | Simple single database setup |
| `docker-compose.dev-only.yml` | Development only | Local development with single dev database |
| `docker-compose.staging.yml` | Staging only | Staging environment testing |
| `docker-compose.production.yml` | Production | Production deployment with security features |
| `docker-compose.multi-env.yml` | Multi-environment | Full multi-environment setup (dev + staging + optional prod-test) |

### Configuration Files

| File | Description |
|------|-------------|
| `config.multi-env.docker.toml` | Multi-environment configuration for Docker |
| `config.staging.docker.toml` | Staging-specific configuration |
| `config.production.docker.toml` | Production-specific configuration |
| `.env.example` | Environment variables template |
| `.env.development` | Development environment variables |
| `.env.staging` | Staging environment variables |

## Quick Start

### 1. Development Environment Only

```bash
# Start development environment
./manage-environments.sh start dev --detached

# Test the environment
./test-multi-env.sh dev

# Check health
./health-check.sh --verbose

# Stop when done
./manage-environments.sh stop dev
```

### 2. Multi-Environment Setup

```bash
# Start multi-environment setup
./manage-environments.sh start multi --detached

# Include production test environment
./manage-environments.sh start multi --with-prod-test --detached

# Test all environments
./test-multi-env.sh multi --verbose

# Check status
./manage-environments.sh status multi
```

### 3. Staging Environment

```bash
# Start staging environment
./manage-environments.sh start staging --detached

# Test staging
./test-multi-env.sh staging

# View logs
./manage-environments.sh logs staging --follow
```

## Environment Variables

### Core MCP Server Variables

- `MCP_DEFAULT_ENVIRONMENT`: Default environment to use when none specified
- `MCP_SERVER_PORT`: Port for MCP server (default: 8080)
- `MCP_LOG_LEVEL`: Logging level (debug, info, warn, error)

### Database Environment Variables

#### Development Environment
- `DEV_MYSQL_ROOT_PASSWORD`: Root password for dev database
- `DEV_MYSQL_DATABASE`: Database name for dev environment
- `DEV_MYSQL_USER`: Username for dev database
- `DEV_MYSQL_PASSWORD`: Password for dev database

#### Staging Environment
- `STAGING_MYSQL_ROOT_PASSWORD`: Root password for staging database
- `STAGING_MYSQL_DATABASE`: Database name for staging environment
- `STAGING_MYSQL_USER`: Username for staging database
- `STAGING_MYSQL_PASSWORD`: Password for staging database

#### Production Test Environment
- `PROD_TEST_MYSQL_ROOT_PASSWORD`: Root password for prod-test database
- `PROD_TEST_MYSQL_DATABASE`: Database name for prod-test environment
- `PROD_TEST_MYSQL_USER`: Username for prod-test database
- `PROD_TEST_MYSQL_PASSWORD`: Password for prod-test database

## Management Scripts

### `manage-environments.sh`

Comprehensive environment management script.

```bash
# Start environments
./manage-environments.sh start dev --detached
./manage-environments.sh start staging --rebuild
./manage-environments.sh start multi --with-prod-test

# Stop environments
./manage-environments.sh stop dev
./manage-environments.sh stop multi

# Check status and health
./manage-environments.sh status multi
./manage-environments.sh health staging

# View logs
./manage-environments.sh logs dev --follow

# Clean up (removes data volumes)
./manage-environments.sh clean dev

# Run tests
./manage-environments.sh test multi
```

### `test-multi-env.sh`

Comprehensive testing script for multi-environment setups.

```bash
# Test specific environment
./test-multi-env.sh dev --verbose
./test-multi-env.sh staging --quick
./test-multi-env.sh multi

# Test all environments
./test-multi-env.sh all --verbose
```

### `health-check.sh`

Health check script that can be used for monitoring.

```bash
# Basic health check
./health-check.sh

# Detailed health check
./health-check.sh --verbose

# Check remote server
./health-check.sh --url http://remote-server:8080 --verbose
```

## Port Mappings

| Environment | MySQL Port | MCP Server Port |
|-------------|------------|-----------------|
| Development | 3306 | 8080 |
| Staging | 3307 | 8080 |
| Production Test | 3308 | 8080 |
| Production | 3306 | 8080 |

## Security Considerations

### Development and Staging
- Uses environment variables for credentials
- Suitable for development and testing environments
- Not recommended for production use

### Production
- Uses Docker secrets for sensitive data
- Resource limits configured
- Enhanced logging and monitoring
- Requires proper secret management setup

### Setting up Production Secrets

```bash
# Create Docker secrets
echo "your_secure_root_password" | docker secret create mysql_root_password -
echo "your_secure_user_password" | docker secret create mysql_password -

# Deploy with secrets
docker stack deploy -c docker-compose.production.yml mysql-mcp-prod
```

## Health Checks

All environments include comprehensive health checks:

- **MCP Server**: HTTP health endpoint with 30s interval
- **MySQL Databases**: MySQL ping with 10s interval
- **Custom Health Check**: Use `health-check.sh` for detailed monitoring

## Troubleshooting

### Common Issues

1. **Port Conflicts**
   ```bash
   # Check what's using the ports
   lsof -i :3306
   lsof -i :3307
   lsof -i :8080
   ```

2. **Database Connection Issues**
   ```bash
   # Check database logs
   ./manage-environments.sh logs dev
   
   # Test direct database connection
   mysql -h localhost -P 3306 -u dev_user -pdev_password
   ```

3. **Environment Variables Not Loading**
   ```bash
   # Verify environment file exists and is readable
   ls -la .env.*
   
   # Check Docker Compose configuration
   docker-compose -f docker-compose.multi-env.yml config
   ```

4. **Health Check Failures**
   ```bash
   # Run detailed health check
   ./health-check.sh --verbose
   
   # Check container status
   docker ps
   docker logs mysql-mcp-server-multi
   ```

### Debugging Commands

```bash
# View effective Docker Compose configuration
docker-compose -f docker-compose.multi-env.yml --env-file .env.example config

# Check container resource usage
docker stats

# Inspect container details
docker inspect mysql-mcp-server-multi

# Access container shell
docker exec -it mysql-mcp-server-multi /bin/bash

# View container logs with timestamps
docker logs -t mysql-mcp-server-multi
```

## Performance Tuning

### Connection Pool Settings

Adjust connection pool settings in configuration files:

```toml
[environments.dev.connection_pool]
max_connections = 10      # Increase for high load
min_connections = 2       # Keep minimum connections
connection_timeout = 30   # Adjust based on network latency
idle_timeout = 600        # How long to keep idle connections
```

### Resource Limits

For production deployments, configure resource limits:

```yaml
deploy:
  resources:
    limits:
      cpus: '2.0'
      memory: 1G
    reservations:
      cpus: '0.5'
      memory: 256M
```

## Monitoring and Logging

### Log Levels

- `debug`: Detailed debugging information
- `info`: General operational information
- `warn`: Warning messages
- `error`: Error messages only

### Log Aggregation

For production environments, consider using log aggregation:

```yaml
logging:
  driver: "json-file"
  options:
    max-size: "10m"
    max-file: "3"
```

## Migration Guide

### From Single to Multi-Environment

1. **Backup existing data**
   ```bash
   docker-compose exec mysql-db mysqldump -u root -p --all-databases > backup.sql
   ```

2. **Update configuration**
   ```bash
   cp config.example.toml config.multi-env.toml
   # Edit configuration to add multiple environments
   ```

3. **Start multi-environment setup**
   ```bash
   ./manage-environments.sh start multi --detached
   ```

4. **Restore data to appropriate environments**
   ```bash
   # Restore to dev environment
   mysql -h localhost -P 3306 -u dev_user -pdev_password < backup.sql
   ```

## Support

For issues and questions:
1. Check the troubleshooting section above
2. Review container logs: `./manage-environments.sh logs <environment>`
3. Run health checks: `./health-check.sh --verbose`
4. Test functionality: `./test-multi-env.sh <environment> --verbose`