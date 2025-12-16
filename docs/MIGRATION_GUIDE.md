# Migration Guide: Single to Multi-Environment Configuration

This guide walks you through migrating from a single-database MySQL MCP Server configuration to a multi-environment setup.

## Overview

The MySQL MCP Server supports both legacy single-database and modern multi-environment configurations. This guide helps you migrate existing setups to take advantage of multi-environment features.

## Before You Start

### Prerequisites
- Existing MySQL MCP Server installation
- Access to your current configuration file
- Database connection details for additional environments
- Backup of your current configuration

### Compatibility
- All existing MCP tools continue to work unchanged
- Legacy configuration format is still supported
- No breaking changes to existing functionality

## Migration Steps

### Step 1: Backup Current Configuration

```bash
# Backup your current config
cp config.toml config.toml.backup
```

### Step 2: Identify Your Current Configuration

#### Legacy Single Database Format
```toml
[server]
port = 8080
log_level = "info"

[database]
host = "localhost"
port = 3306
username = "myuser"
password = "mypassword"
database = "mydatabase"
connection_timeout = 30
max_connections = 10

[mcp]
protocol_version = "2024-11-05"
server_name = "mysql-mcp-server"
server_version = "0.1.0"
```

### Step 3: Convert to Multi-Environment Format

#### Option A: Minimal Migration (Keep Current Database as Default)

```toml
# Set your current database as the default environment
default_environment = "current"

[server]
port = 8080
log_level = "info"

[mcp]
protocol_version = "2024-11-05"
server_name = "mysql-mcp-server"
server_version = "0.1.0"

# Convert your existing database to an environment
[environments.current]
name = "current"
description = "Migrated from legacy single database configuration"
enabled = true

[environments.current.database]
host = "localhost"
port = 3306
username = "myuser"
password = "mypassword"
database = "mydatabase"
connection_timeout = 30

[environments.current.connection_pool]
max_connections = 10
min_connections = 1
connection_timeout = 30
idle_timeout = 600
```

#### Option B: Full Multi-Environment Setup

```toml
default_environment = "dev"

[server]
port = 8080
log_level = "info"

[mcp]
protocol_version = "2024-11-05"
server_name = "mysql-mcp-server"
server_version = "0.1.0"

# Development environment (your current database)
[environments.dev]
name = "dev"
description = "Development environment"
enabled = true

[environments.dev.database]
host = "localhost"
port = 3306
username = "myuser"
password = "mypassword"
database = "mydatabase"
connection_timeout = 30

[environments.dev.connection_pool]
max_connections = 10
min_connections = 1
connection_timeout = 30
idle_timeout = 600

# Add additional environments
[environments.staging]
name = "staging"
description = "Staging environment"
enabled = true

[environments.staging.database]
host = "staging-db.company.com"
port = 3306
username = "staging_user"
password = "staging_password"
database = "staging_database"
connection_timeout = 30

[environments.staging.connection_pool]
max_connections = 8
min_connections = 2
connection_timeout = 30
idle_timeout = 600

[environments.prod]
name = "prod"
description = "Production environment"
enabled = false  # Start disabled for safety

[environments.prod.database]
host = "prod-db.company.com"
port = 3306
username = "prod_user"
password = "prod_password"
database = "prod_database"
connection_timeout = 30

[environments.prod.connection_pool]
max_connections = 20
min_connections = 5
connection_timeout = 30
idle_timeout = 600
```

### Step 4: Test the Migration

1. **Start the server with new configuration:**
```bash
# Test configuration validation
mysql-mcp-server --config config.toml --validate-only

# Start server
mysql-mcp-server --config config.toml
```

2. **Verify legacy tools still work:**
```json
{
  "name": "test_connection",
  "arguments": {}
}
```

3. **Test new multi-environment tools:**
```json
{
  "name": "list_environments",
  "arguments": {}
}
```

### Step 5: Update Your Applications

#### Gradual Migration Approach

**Phase 1: Keep using legacy tools**
- No changes needed to existing applications
- Legacy tools automatically use the default environment

**Phase 2: Start using environment-specific tools**
```json
// Old way
{
  "name": "execute_query",
  "arguments": {
    "sql": "SELECT COUNT(*) FROM users"
  }
}

// New way - explicit environment
{
  "name": "execute_query_env",
  "arguments": {
    "sql": "SELECT COUNT(*) FROM users",
    "environment": "dev"
  }
}
```

**Phase 3: Leverage multi-environment features**
```json
// Compare data across environments
{
  "name": "execute_query_multi_env",
  "arguments": {
    "sql": "SELECT COUNT(*) FROM users",
    "environments": ["dev", "staging", "prod"]
  }
}
```

## Configuration Migration Examples

### Example 1: Development Team Setup

**Before (Single Database):**
```toml
[database]
host = "localhost"
port = 3306
username = "dev_user"
password = "dev_pass"
database = "app_dev"
```

**After (Multi-Environment):**
```toml
default_environment = "local"

[environments.local]
name = "local"
enabled = true

[environments.local.database]
host = "localhost"
port = 3306
username = "dev_user"
password = "dev_pass"
database = "app_dev"

[environments.shared_dev]
name = "shared_dev"
enabled = true

[environments.shared_dev.database]
host = "dev-db.company.com"
port = 3306
username = "shared_dev_user"
password = "shared_dev_pass"
database = "app_shared_dev"
```

### Example 2: Production Deployment

**Before:**
```toml
[database]
host = "prod-db.company.com"
port = 3306
username = "prod_user"
password = "prod_pass"
database = "app_prod"
```

**After:**
```toml
default_environment = "prod"

[environments.prod]
name = "prod"
enabled = true

[environments.prod.database]
host = "prod-db.company.com"
port = 3306
username = "prod_user"
password = "prod_pass"
database = "app_prod"

[environments.prod.connection_pool]
max_connections = 20
min_connections = 5
connection_timeout = 30
idle_timeout = 600

# Add read replica for reporting
[environments.prod_readonly]
name = "prod_readonly"
enabled = true

[environments.prod_readonly.database]
host = "prod-readonly-db.company.com"
port = 3306
username = "readonly_user"
password = "readonly_pass"
database = "app_prod"
```

## Rollback Plan

If you need to rollback to the single-database configuration:

1. **Stop the server**
2. **Restore backup configuration:**
```bash
cp config.toml.backup config.toml
```
3. **Restart the server**

## Validation Checklist

After migration, verify:

- [ ] Server starts successfully
- [ ] Legacy tools work (`test_connection`, `execute_query`)
- [ ] New tools work (`list_environments`, `execute_query_env`)
- [ ] All environments show as healthy in `list_environments`
- [ ] Database connections work for all environments
- [ ] Application functionality is unchanged

## Common Migration Issues

### Issue: "Both legacy and multi-environment config found"
**Solution:** Remove the `[database]` section when using `[environments]`

### Issue: Default environment not working
**Solution:** Ensure `default_environment` matches an enabled environment name

### Issue: Connection pool errors after migration
**Solution:** Adjust connection pool settings for your workload:
```toml
[environments.myenv.connection_pool]
max_connections = 15  # Adjust based on usage
min_connections = 3
```

## Best Practices After Migration

1. **Start with conservative pool sizes** and adjust based on usage
2. **Keep production environments disabled initially** until thoroughly tested
3. **Use environment-specific tools** for better error handling and monitoring
4. **Implement health checks** using the new monitoring tools
5. **Document your environment naming conventions** for your team

## Getting Help

If you encounter issues during migration:

1. Check the [Troubleshooting Guide](TROUBLESHOOTING.md)
2. Validate your configuration with `--validate-only` flag
3. Review server logs for specific error messages
4. Test connectivity to each database manually

Remember: The migration is backward compatible, so you can always rollback if needed.