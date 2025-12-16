# MySQL MCP Server - Multi-Environment Tools Reference

This document provides a comprehensive reference for all MCP tools available in the MySQL MCP Server with multi-environment support.

## Overview

The MySQL MCP Server provides two categories of tools:
1. **Legacy Tools**: Original single-database tools (maintained for backward compatibility)
2. **Enhanced Multi-Environment Tools**: New tools that support multiple database environments

## Legacy Tools (Single Database)

These tools work with the default environment or legacy single-database configuration:

### execute_query
Execute read-only SQL queries against the default database.

**Parameters:**
- `sql` (required): SQL query string (SELECT, SHOW, DESCRIBE, EXPLAIN only)
- `parameters` (optional): Array of query parameters
- `stream_results` (optional): Boolean to enable streaming for large results

**Example:**
```json
{
  "name": "execute_query",
  "arguments": {
    "sql": "SELECT * FROM users LIMIT 10"
  }
}
```

### test_connection
Test the connection to the default database.

**Parameters:** None

### list_databases
List all databases in the default environment.

**Parameters:** None

### list_tables
List tables in a specific database.

**Parameters:**
- `database` (optional): Database name (uses current database if not specified)

### describe_table
Get detailed table structure information.

**Parameters:**
- `table` (required): Table name
- `database` (optional): Database name

### list_columns
List columns in a specific table.

**Parameters:**
- `table` (required): Table name
- `database` (optional): Database name

## Enhanced Multi-Environment Tools

These tools support multiple database environments and provide advanced functionality:

### execute_query_env
Execute a query against a specific environment.

**Parameters:**
- `sql` (required): SQL query string
- `environment` (required): Target environment name
- `parameters` (optional): Array of query parameters
- `stream_results` (optional): Enable streaming

**Example:**
```json
{
  "name": "execute_query_env",
  "arguments": {
    "sql": "SELECT COUNT(*) FROM orders",
    "environment": "prod"
  }
}
```
### execute_query_multi_env
Execute the same query against multiple environments for comparison.

**Parameters:**
- `sql` (required): SQL query string
- `environments` (required): Array of environment names
- `parameters` (optional): Array of query parameters
- `compare_results` (optional): Enable result comparison

**Example:**
```json
{
  "name": "execute_query_multi_env",
  "arguments": {
    "sql": "SELECT COUNT(*) as user_count FROM users",
    "environments": ["dev", "staging", "prod"],
    "compare_results": true
  }
}
```

### list_environments
List all configured database environments with their status and connection information.

**Parameters:**
- `include_disabled` (optional): Boolean to include disabled environments (default: false)

**Example:**
```json
{
  "name": "list_environments",
  "arguments": {
    "include_disabled": true
  }
}
```

**Response includes:**
- Environment name and description
- Status (enabled/disabled/invalid)
- Whether it's the default environment
- Connection information (host, port, database, username)
- Pool configuration (max/min connections, timeouts)
- Total environment count

**Sample Response:**
```json
{
  "environments": [
    {
      "name": "uat",
      "description": "User Acceptance Testing environment - AWS RDS Aurora",
      "status": "enabled",
      "is_default": true,
      "is_legacy": false,
      "connection_info": {
        "host": "dcs-uat-rds-aurora-cluster.cluster-xxx.amazonaws.com",
        "port": 3306,
        "database": "information_schema",
        "username": "web3-rds",
        "password_configured": true
      },
      "pool_config": {
        "max_connections": 10,
        "min_connections": 2,
        "connection_timeout": 30,
        "idle_timeout": 600
      }
    }
  ],
  "total_count": 1,
  "default_environment": "uat"
}
```

### list_databases_env
List databases in a specific environment.

**Parameters:**
- `environment` (required): Target environment name

### list_databases_all_env
List databases across all environments.

**Parameters:**
- `environments` (optional): Array of specific environments (defaults to all)

### list_tables_env
List tables in a specific environment and database.

**Parameters:**
- `environment` (required): Target environment name
- `database` (optional): Database name

### describe_table_env
Get table structure from a specific environment.

**Parameters:**
- `environment` (required): Target environment name
- `table` (required): Table name
- `database` (optional): Database name

### compare_schema
Compare schema differences across environments.

**Parameters:**
- `environments` (required): Array of environments to compare
- `table` (optional): Specific table to compare
- `database` (optional): Specific database to compare

**Example:**
```json
{
  "name": "compare_schema",
  "arguments": {
    "environments": ["staging", "prod"],
    "table": "users"
  }
}
```

### health_check_env
Perform detailed health check on a specific environment.

**Parameters:**
- `environment` (required): Target environment name

**Response includes:**
- Connection test results
- Performance metrics
- Pool status
- Diagnostic information

### test_connection_env
Test connection to a specific environment.

**Parameters:**
- `environment` (required): Target environment name

## Tool Response Formats

### Single Environment Response
```json
{
  "environment": "dev",
  "execution_time_ms": 45,
  "affected_rows": null,
  "columns": [...],
  "rows": [...],
  "error": null
}
```

### Multi-Environment Response
```json
{
  "results": {
    "dev": { /* single environment response */ },
    "staging": { /* single environment response */ }
  },
  "comparison": {
    "identical": false,
    "differences": [...],
    "row_count_comparison": {
      "dev": 150,
      "staging": 148
    }
  },
  "summary": {
    "total_environments": 2,
    "successful_environments": 2,
    "failed_environments": 0
  }
}
```

## Error Handling

All tools provide consistent error handling with environment context:

```json
{
  "error": {
    "type": "connection_error",
    "message": "Unable to connect to environment 'prod'",
    "environment": "prod",
    "details": "Connection timeout after 30 seconds"
  }
}
```

## Best Practices

1. **Environment Selection**: Always specify the environment explicitly for production queries
2. **Query Comparison**: Use `execute_query_multi_env` to verify data consistency across environments
3. **Health Monitoring**: Regularly use `health_check_env` to monitor environment status
4. **Schema Validation**: Use `compare_schema` before deployments to ensure schema consistency
5. **Error Handling**: Always check the `error` field in responses before processing results