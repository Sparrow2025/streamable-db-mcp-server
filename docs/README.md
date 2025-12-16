# MySQL MCP Server - Multi-Environment Support

A powerful MySQL Model Context Protocol (MCP) server with comprehensive multi-environment support, enabling seamless database operations across development, staging, and production environments.

## Features

### 🌍 Multi-Environment Support
- **Simultaneous Connections**: Connect to multiple database environments (dev, staging, prod) simultaneously
- **Environment-Aware Tools**: Enhanced MCP tools with environment-specific operations
- **Graceful Startup**: Server starts even if some environments are unavailable
- **Health Monitoring**: Comprehensive health checks and monitoring for all environments

### 🔧 Enhanced MCP Tools
- **Legacy Compatibility**: All existing tools continue to work unchanged
- **Environment-Specific Queries**: Execute queries against specific environments
- **Multi-Environment Queries**: Run the same query across multiple environments for comparison
- **Schema Comparison**: Compare database schemas across environments
- **Environment Management**: List, monitor, and manage all configured environments

### 🚀 Robust Architecture
- **Connection Pooling**: Per-environment connection pools with automatic management
- **Fault Tolerance**: Automatic reconnection with exponential backoff
- **Streaming Support**: Large result set streaming across multiple environments
- **Secure Logging**: Environment-aware logging with credential protection

## Quick Start

### 1. Configuration

Create a multi-environment configuration file:

```toml
# config.toml
default_environment = "dev"

[server]
port = 8080
log_level = "info"

[mcp]
protocol_version = "2024-11-05"
server_name = "mysql-mcp-server"
server_version = "0.1.0"

[environments.dev]
name = "dev"
enabled = true

[environments.dev.database]
host = "localhost"
port = 3306
username = "dev_user"
password = "dev_password"
database = "dev_db"

[environments.prod]
name = "prod"
enabled = true

[environments.prod.database]
host = "prod-db.company.com"
port = 3306
username = "prod_user"
password = "prod_password"
database = "prod_db"
```

### 2. Start the Server

```bash
# Start with multi-environment configuration
mysql-mcp-server --config config.toml

# Or using Docker
docker run -p 8080:8080 -v $(pwd)/config.toml:/app/config.toml mysql-mcp-server
```

### 3. Use Multi-Environment Tools

```json
// List all environments
{
  "name": "list_environments",
  "arguments": {}
}

// Query specific environment
{
  "name": "execute_query_env",
  "arguments": {
    "sql": "SELECT COUNT(*) FROM users",
    "environment": "prod"
  }
}

// Compare across environments
{
  "name": "execute_query_multi_env",
  "arguments": {
    "sql": "SELECT COUNT(*) FROM users",
    "environments": ["dev", "staging", "prod"]
  }
}
```

## Documentation

### 📚 Comprehensive Guides
- **[MCP Tools Reference](MCP_TOOLS_REFERENCE.md)**: Complete reference for all available tools
- **[Migration Guide](MIGRATION_GUIDE.md)**: Step-by-step migration from single to multi-environment
- **[Troubleshooting Guide](TROUBLESHOOTING.md)**: Common issues and solutions
- **[Docker Setup Guide](../docker/README.md)**: Docker deployment with multiple environments

### 🔧 Configuration Examples
- **[Complete Example](../config.multi-env.complete.example.toml)**: Full configuration with all options
- **[Minimal Example](../config.multi-env.minimal.example.toml)**: Minimal multi-environment setup
- **[Docker Example](../config.docker.example.toml)**: Docker-optimized configuration

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    MCP Server                               │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │   MCP Protocol  │  │  Query Router   │  │ Environment │ │
│  │    Handler      │  │                 │  │  Manager    │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                Connection Pool Manager                      │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │ DEV Pool    │  │ STAGING Pool│  │ PROD Pool   │   ...   │
│  │ (MySQL)     │  │ (MySQL)     │  │ (MySQL)     │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
└─────────────────────────────────────────────────────────────┘
           │                │                │
           ▼                ▼                ▼
    ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
    │   DEV DB    │  │ STAGING DB  │  │   PROD DB   │
    └─────────────┘  └─────────────┘  └─────────────┘
```

## Key Components

### Environment Manager
- Loads and validates environment configurations
- Manages environment metadata and credentials
- Provides environment discovery and status reporting

### Connection Pool Manager
- Maintains separate connection pools per environment
- Handles connection lifecycle and health monitoring
- Implements automatic reconnection with exponential backoff

### Query Router
- Routes queries to appropriate environments
- Handles multi-environment query execution
- Aggregates and formats results from multiple environments

### Enhanced MCP Tools
- Environment-aware versions of all existing tools
- New multi-environment specific tools
- Schema comparison and environment management tools

## Benefits

### 🎯 Developer Productivity
- **Single Interface**: Access all environments through one server
- **Data Comparison**: Easily compare data across environments
- **Schema Validation**: Verify schema consistency before deployments
- **Environment Isolation**: Safe operations with clear environment context

### 🛡️ Operational Safety
- **Graceful Degradation**: Server continues operating with partial failures
- **Health Monitoring**: Comprehensive monitoring and alerting
- **Secure Logging**: Environment context without credential exposure
- **Connection Management**: Automatic reconnection and resource management

### 🔄 Migration Friendly
- **Backward Compatibility**: Existing tools and configurations continue to work
- **Gradual Migration**: Migrate from single to multi-environment incrementally
- **Legacy Support**: Full support for existing single-database setups

## Use Cases

### Development Teams
- **Local Development**: Connect to local and shared development databases
- **Testing**: Compare test results across different environments
- **Debugging**: Investigate issues by comparing data across environments

### DevOps and SRE
- **Deployment Validation**: Verify deployments by comparing schemas and data
- **Environment Monitoring**: Monitor health and performance of all environments
- **Incident Response**: Quickly access multiple environments during incidents

### Data Teams
- **Data Validation**: Ensure data consistency across environments
- **Migration Testing**: Validate data migrations before production deployment
- **Reporting**: Generate reports that span multiple environments

## Getting Started

1. **[Install](../README.md#installation)** the MySQL MCP Server
2. **[Configure](MIGRATION_GUIDE.md)** your multi-environment setup
3. **[Deploy](../docker/README.md)** using Docker (optional)
4. **[Explore](MCP_TOOLS_REFERENCE.md)** the enhanced MCP tools
5. **[Monitor](TROUBLESHOOTING.md)** your environments

## Support

- **Documentation**: Comprehensive guides and references
- **Examples**: Real-world configuration examples
- **Troubleshooting**: Common issues and solutions
- **Migration**: Step-by-step migration assistance

---

**Ready to get started?** Check out the [Migration Guide](MIGRATION_GUIDE.md) to upgrade your existing setup or the [MCP Tools Reference](MCP_TOOLS_REFERENCE.md) to explore all available features.