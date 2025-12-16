# MySQL MCP Server - Multi-Environment Troubleshooting Guide

This guide helps you diagnose and resolve common issues with multi-environment MySQL MCP Server setup.

## Common Issues and Solutions

### 1. Server Startup Issues

#### Problem: "No enabled environments found"
**Symptoms:**
- Server fails to start
- Error message mentions no enabled environments

**Solution:**
```toml
# Ensure at least one environment is enabled
[environments.dev]
name = "dev"
enabled = true  # Make sure this is true
```

#### Problem: "Default environment not found"
**Symptoms:**
- Server starts but tools fail
- Error about default environment

**Solution:**
```toml
# Either set a valid default environment
default_environment = "dev"

# Or remove the default_environment setting to use the first enabled environment
```

#### Problem: "No healthy environments available"
**Symptoms:**
- Server starts but all environments are unhealthy
- Connection errors for all environments

**Diagnosis Steps:**
1. Check network connectivity to database hosts
2. Verify database credentials
3. Ensure databases are running and accessible
4. Check firewall settings

### 2. Connection Issues

#### Problem: Connection timeouts
**Symptoms:**
- "Connection timeout" errors
- Slow query responses

**Solutions:**
```toml
[environments.myenv.database]
connection_timeout = 60  # Increase timeout

[environments.myenv.connection_pool]
connection_timeout = 60  # Also increase pool timeout
```

#### Problem: "Pool exhausted" errors
**Symptoms:**
- "No connections available" errors
- High connection usage

**Solutions:**
```toml
[environments.myenv.connection_pool]
max_connections = 20     # Increase pool size
min_connections = 5      # Increase minimum connections
idle_timeout = 300       # Reduce idle timeout
```

### 3. Configuration Issues

#### Problem: Invalid environment names
**Symptoms:**
- Configuration validation errors
- Environment names rejected

**Solution:**
Environment names must contain only alphanumeric characters, hyphens, and underscores:
```toml
# ✅ Valid names
[environments.dev-1]
[environments.staging_env]
[environments.prod2]

# ❌ Invalid names
[environments.dev@test]  # Contains @
[environments.prod env]  # Contains space
```

#### Problem: Missing required configuration
**Symptoms:**
- "Missing required parameter" errors
- Configuration validation failures

**Required fields checklist:**
```toml
[environments.myenv]
name = "myenv"           # Required: must match section name
enabled = true           # Required: boolean

[environments.myenv.database]
host = "localhost"       # Required: database host
port = 3306             # Required: database port
username = "user"        # Required: database username
password = "pass"        # Required: database password
database = "dbname"      # Required: database name
```

### 4. Multi-Environment Query Issues

#### Problem: Environment not found in multi-env queries
**Symptoms:**
- "Environment 'xyz' not found" errors
- Queries fail for specific environments

**Solution:**
1. Check environment name spelling
2. Verify environment is enabled
3. Use `list_environments` tool to see available environments

#### Problem: Inconsistent results across environments
**Symptoms:**
- Different row counts between environments
- Schema differences

**Diagnosis:**
```json
{
  "name": "compare_schema",
  "arguments": {
    "environments": ["dev", "prod"],
    "table": "users"
  }
}
```

### 5. Performance Issues

#### Problem: Slow query execution
**Symptoms:**
- High execution times
- Timeouts on large queries

**Solutions:**
1. Enable streaming for large result sets:
```json
{
  "name": "execute_query_env",
  "arguments": {
    "sql": "SELECT * FROM large_table",
    "environment": "prod",
    "stream_results": true
  }
}
```

2. Optimize connection pool settings:
```toml
[environments.myenv.connection_pool]
max_connections = 15
min_connections = 3
connection_timeout = 45
idle_timeout = 900
```

#### Problem: High memory usage
**Symptoms:**
- Server memory consumption grows
- Out of memory errors

**Solutions:**
1. Reduce connection pool sizes
2. Enable streaming for large queries
3. Implement query result limits

## Diagnostic Commands

### Check Environment Status
```json
{
  "name": "list_environments",
  "arguments": {}
}
```

### Test Specific Environment
```json
{
  "name": "health_check_env",
  "arguments": {
    "environment": "prod"
  }
}
```

### Compare Environments
```json
{
  "name": "execute_query_multi_env",
  "arguments": {
    "sql": "SELECT 1 as test",
    "environments": ["dev", "staging", "prod"]
  }
}
```

## Log Analysis

### Enable Debug Logging
```toml
[server]
log_level = "debug"
```

### Key Log Messages to Look For

**Successful startup:**
```
✅ Environment 'dev': Healthy and ready
✅ Environment 'prod': Healthy and ready
Multi-environment initialization completed successfully
```

**Connection issues:**
```
❌ Environment 'prod': Unhealthy - Connection timeout
⚠️  Starting with partial environment availability
```

**Configuration problems:**
```
Configuration error for 'environments': Environment validation failed
```

## Getting Help

### Information to Collect
1. Configuration file (with passwords masked)
2. Server logs (last 100 lines)
3. Environment status from `list_environments`
4. Network connectivity test results

### Log Locations
- Docker: `docker logs <container_name>`
- Direct run: Console output or configured log file
- Systemd: `journalctl -u mysql-mcp-server`

### Useful Commands
```bash
# Test database connectivity
mysql -h <host> -P <port> -u <username> -p<password> -e "SELECT 1"

# Check network connectivity
telnet <host> <port>

# Verify DNS resolution
nslookup <host>
```