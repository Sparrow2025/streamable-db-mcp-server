# Design Document - Multi-Database Connection Support

## Overview

This design extends the MySQL MCP Server to support simultaneous connections to multiple database environments. Instead of connecting to a single database, the server will maintain a pool of connections to different environments (dev, sit, uat, prod), allowing developers to query and compare data across environments within a single MCP session.

## Architecture

### High-Level Architecture

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
│  │ DEV Pool    │  │ SIT Pool    │  │ UAT Pool    │   ...   │
│  │ (MySQL)     │  │ (MySQL)     │  │ (MySQL)     │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
└─────────────────────────────────────────────────────────────┘
           │                │                │
           ▼                ▼                ▼
    ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
    │   DEV DB    │  │   SIT DB    │  │   UAT DB    │
    │             │  │             │  │             │
    └─────────────┘  └─────────────┘  └─────────────┘
```

## Components and Interfaces

### 1. Environment Manager

**Responsibilities:**
- Load and validate environment configurations
- Manage environment metadata and connection parameters
- Provide environment discovery and listing capabilities

**Interface:**
```rust
pub struct EnvironmentManager {
    environments: HashMap<String, EnvironmentConfig>,
    default_environment: Option<String>,
}

impl EnvironmentManager {
    pub fn load_from_config(config: &Config) -> Result<Self, ConfigError>;
    pub fn get_environment(&self, name: &str) -> Option<&EnvironmentConfig>;
    pub fn list_environments(&self) -> Vec<&str>;
    pub fn validate_environment(&self, name: &str) -> Result<(), ValidationError>;
}
```

### 2. Connection Pool Manager

**Responsibilities:**
- Maintain separate connection pools for each environment
- Handle connection lifecycle (creation, health checks, reconnection)
- Provide connection isolation and resource management

**Interface:**
```rust
pub struct ConnectionPoolManager {
    pools: HashMap<String, ConnectionPool>,
    health_checker: HealthChecker,
}

impl ConnectionPoolManager {
    pub async fn initialize(environments: &EnvironmentManager) -> Result<Self, PoolError>;
    pub async fn get_connection(&self, env: &str) -> Result<Connection, PoolError>;
    pub async fn health_check(&self, env: Option<&str>) -> HealthStatus;
    pub async fn reconnect(&self, env: &str) -> Result<(), PoolError>;
}
```

### 3. Query Router

**Responsibilities:**
- Route queries to appropriate environment(s)
- Handle multi-environment query execution
- Aggregate and format results from multiple environments

**Interface:**
```rust
pub struct QueryRouter {
    pool_manager: Arc<ConnectionPoolManager>,
    environment_manager: Arc<EnvironmentManager>,
}

impl QueryRouter {
    pub async fn execute_query(&self, query: &QueryRequest) -> Result<QueryResponse, QueryError>;
    pub async fn execute_multi_env_query(&self, query: &MultiEnvQueryRequest) -> Result<MultiEnvQueryResponse, QueryError>;
    pub async fn compare_across_environments(&self, query: &str, envs: &[String]) -> Result<ComparisonResult, QueryError>;
}
```

### 4. Enhanced MCP Tools

**New Environment-Aware Tools:**
- `execute_query_env` - Execute query against specific environment
- `execute_query_multi_env` - Execute query against multiple environments
- `list_databases_env` - List databases in specific environment
- `list_databases_all_env` - List databases across all environments
- `compare_schema` - Compare schema across environments
- `health_check_env` - Check health of specific environment
- `list_environments` - List all configured environments

## Data Models

### Environment Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub name: String,
    pub description: Option<String>,
    pub database: DatabaseConfig,
    pub connection_pool: PoolConfig,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    pub connection_timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout: u64,
    pub idle_timeout: u64,
}
```

### Query Request Models

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryRequest {
    pub sql: String,
    pub environment: Option<String>,
    pub parameters: Option<Vec<serde_json::Value>>,
    pub stream_results: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MultiEnvQueryRequest {
    pub sql: String,
    pub environments: Vec<String>,
    pub parameters: Option<Vec<serde_json::Value>>,
    pub compare_results: Option<bool>,
}
```

### Response Models

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResponse {
    pub environment: String,
    pub execution_time_ms: u64,
    pub affected_rows: Option<u64>,
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Row>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MultiEnvQueryResponse {
    pub results: HashMap<String, QueryResponse>,
    pub comparison: Option<ComparisonResult>,
    pub summary: ExecutionSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub identical: bool,
    pub differences: Vec<EnvironmentDifference>,
    pub row_count_comparison: HashMap<String, u64>,
}
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Environment Connection Initialization
*For any* valid multi-environment configuration, when the MCP Server starts, all configured and enabled environments should have established database connections
**Validates: Requirements 1.1**

### Property 2: Query Environment Routing
*For any* valid SQL query and environment identifier, when executing a query with environment specification, the query should be routed to and executed against the specified environment only
**Validates: Requirements 1.2**

### Property 3: Default Environment Behavior
*For any* query request without environment specification, the system should either use a configured default environment or return a clear error message consistently
**Validates: Requirements 1.3**

### Property 4: Environment Listing Completeness
*For any* system state, when listing environments, all configured environments should be included with accurate connection status information
**Validates: Requirements 1.4**

### Property 5: Fault Tolerance
*For any* subset of environments experiencing connection failures, the system should continue operating normally with the remaining healthy connections
**Validates: Requirements 1.5**

### Property 6: Multi-Environment Query Consistency
*For any* SQL query and list of environments, when executing a multi-environment query, the same SQL should be executed against all specified environments
**Validates: Requirements 2.1**

### Property 7: Result Environment Identification
*For any* multi-environment query result, each result set should be clearly tagged with its originating environment identifier
**Validates: Requirements 2.2**

### Property 8: Comparison Result Structure
*For any* multi-environment query with comparison enabled, the comparison results should follow a consistent structured format that clearly identifies differences
**Validates: Requirements 2.3**

### Property 9: Partial Result Handling
*For any* multi-environment query where some environments fail, the system should return partial results from successful environments along with error information for failed ones
**Validates: Requirements 2.4**

### Property 10: Response Format Consistency
*For any* multi-environment query, the response structure should be consistent regardless of which specific environments are queried
**Validates: Requirements 2.5**

### Property 11: Health Information Completeness
*For any* health check request, the system should provide health information for each configured environment
**Validates: Requirements 3.1**

### Property 12: Reconnection Behavior
*For any* lost database connection, the system should attempt reconnection using exponential backoff strategy
**Validates: Requirements 3.2**

### Property 13: Connection Test Detail
*For any* connection test request, the system should provide detailed connectivity information for each specified environment
**Validates: Requirements 3.3**

### Property 14: Error Logging Context
*For any* connection error or unhealthy state, the system should log appropriate error messages that include environment context
**Validates: Requirements 3.4**

### Property 15: Connection Pool Limits
*For any* environment configuration with specified connection limits, the connection pool should respect the per-environment limits and timeout settings
**Validates: Requirements 3.5**

### Property 16: Environment-Aware Tool Availability
*For any* existing MCP tool, there should be an environment-aware version that accepts environment parameters
**Validates: Requirements 4.1**

### Property 17: Tool Parameter Environment Context
*For any* environment-specific tool execution, the tool parameters should include environment information when an environment is specified
**Validates: Requirements 4.2**

### Property 18: Environment Filtering Support
*For any* listing operation (databases, tables), the system should support filtering by environment or displaying results from all environments
**Validates: Requirements 4.3**

### Property 19: Schema Comparison Capability
*For any* database object description request, the system should support comparing schema differences across multiple environments
**Validates: Requirements 4.4**

### Property 20: Concurrent Streaming Support
*For any* streaming query request against multiple environments, the system should support simultaneous streaming from all specified environments
**Validates: Requirements 4.5**

### Property 21: Environment-Specific Configuration
*For any* multi-environment configuration, each environment should have its own separate connection parameters that don't interfere with other environments
**Validates: Requirements 5.1**

### Property 22: Credential Isolation
*For any* environment's credentials, they should be completely isolated from other environments and not accessible when working with different environments
**Validates: Requirements 5.2**

### Property 23: Configuration Validation Completeness
*For any* environment configuration, all required parameters should be validated and missing parameters should result in clear validation errors
**Validates: Requirements 5.3**

### Property 24: Secure Logging with Context
*For any* logged operation, the logs should include environment context while ensuring sensitive information (passwords, connection strings) is never exposed
**Validates: Requirements 5.4**

### Property 25: Clear Configuration Error Messages
*For any* configuration error, the error message should clearly identify which specific environment has the configuration problem
**Validates: Requirements 5.5**

## Error Handling

### Error Categories

1. **Configuration Errors**
   - Invalid environment configuration
   - Missing required parameters
   - Credential validation failures

2. **Connection Errors**
   - Database connection failures
   - Network connectivity issues
   - Authentication failures

3. **Query Errors**
   - Invalid SQL syntax
   - Permission denied
   - Environment not found

4. **Multi-Environment Errors**
   - Partial execution failures
   - Environment mismatch errors
   - Comparison failures

### Error Response Format

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct EnvironmentError {
    pub error_type: ErrorType,
    pub environment: Option<String>,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
```

## Testing Strategy

### Unit Testing
- Test environment configuration loading and validation
- Test connection pool management for individual environments
- Test query routing logic
- Test error handling for various failure scenarios
- Test MCP tool parameter validation and execution

### Property-Based Testing
- Use **proptest** crate for Rust property-based testing
- Configure each property-based test to run a minimum of 100 iterations
- Each property-based test will be tagged with comments referencing the corresponding correctness property

**Property-Based Test Requirements:**
- Each correctness property must be implemented by a single property-based test
- Tests must be tagged with format: `**Feature: multi-environment-support, Property {number}: {property_text}**`
- Tests should generate random but valid inputs within the domain constraints
- Tests should avoid mocking where possible to validate real functionality

### Integration Testing
- Test end-to-end multi-environment query execution
- Test connection failover and recovery scenarios
- Test MCP protocol compliance with multiple environments
- Test Docker deployment with multiple environment configurations

### Performance Testing
- Test concurrent query execution across multiple environments
- Test connection pool performance under load
- Test memory usage with multiple active connections
- Test query response times with environment routing overhead