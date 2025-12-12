# Design Document

## Overview

The Streamable MySQL MCP Server is a Rust-based implementation that provides MySQL database access through the Model Context Protocol. The server uses the `rmcp` crate (version 0.8) with streamable HTTP transport to enable efficient query execution and result streaming. The design prioritizes simplicity, performance, and reliability while maintaining full MCP protocol compliance.

## Architecture

The system follows a layered architecture with clear separation of concerns:

```
┌─────────────────────────────────────┐
│           MCP Client                │
└─────────────────┬───────────────────┘
                  │ HTTP/MCP Protocol
┌─────────────────▼───────────────────┐
│        MCP Server Layer             │
│  (rmcp with streamable transport)   │
└─────────────────┬───────────────────┘
                  │ Internal API
┌─────────────────▼───────────────────┐
│      Query Processing Layer         │
│   (SQL parsing, validation)         │
└─────────────────┬───────────────────┘
                  │ Database API
┌─────────────────▼───────────────────┐
│    Connection Management Layer      │
│      (Connection pooling)           │
└─────────────────┬───────────────────┘
                  │ MySQL Protocol
┌─────────────────▼───────────────────┐
│         MySQL Database              │
└─────────────────────────────────────┘
```

## Components and Interfaces

### MCP Server Component
- **Responsibility**: Handle MCP protocol communication and HTTP transport
- **Key Dependencies**: `rmcp` crate with server, macros, and transport-streamable-http-server features
- **Interface**: Exposes MCP tools for database operations
- **Implementation**: Uses rmcp's streamable HTTP server for efficient data transfer

### Query Processor Component
- **Responsibility**: Parse, validate, and execute SQL queries
- **Key Dependencies**: `sqlx` for MySQL connectivity, `serde` for serialization
- **Interface**: Accepts SQL strings, returns structured results or errors
- **Implementation**: Handles different query types (SELECT, INSERT, UPDATE, DELETE)

### Connection Manager Component
- **Responsibility**: Manage single MySQL database connection
- **Key Dependencies**: `sqlx::MySqlConnection`
- **Interface**: Provides simple connection management
- **Implementation**: Single connection with basic reconnection logic

### Result Streamer Component
- **Responsibility**: Stream large query results efficiently
- **Key Dependencies**: `tokio::stream`, `futures`
- **Interface**: Converts query results into streamable format
- **Implementation**: Uses async streams to send results incrementally

## Data Models

### Connection Configuration
```rust
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub database_url: String,
}
```

### Query Request
```rust
#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub sql: String,
    pub parameters: Option<Vec<serde_json::Value>>,
    pub stream_results: bool,
}
```

### Query Result
```rust
#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Row>,
    pub affected_rows: Option<u64>,
    pub execution_time_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

#[derive(Debug, Serialize)]
pub struct Row {
    pub values: Vec<serde_json::Value>,
}
```

### Streaming Result Chunk
```rust
#[derive(Debug, Serialize)]
pub struct ResultChunk {
    pub chunk_id: u64,
    pub rows: Vec<Row>,
    pub is_final: bool,
    pub total_rows: Option<u64>,
}
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

**Property 1: Connection establishment success**
*For any* valid connection configuration, the MCP Server should successfully establish a connection to the MySQL Database
**Validates: Requirements 1.1**

**Property 2: Connection parameter validation**
*For any* connection parameters provided, the MCP Server should validate the format before attempting connection
**Validates: Requirements 1.2**

**Property 3: Connection failure error reporting**
*For any* invalid connection attempt, the MCP Server should return a descriptive error message to the client
**Validates: Requirements 1.3**

**Property 4: Connection pool management**
*For any* number of concurrent client connections, the MCP Server should manage them through the Connection Pool
**Validates: Requirements 1.4**

**Property 5: SELECT query execution**
*For any* valid SELECT query, the MCP Server should execute it against the MySQL Database and return a properly formatted Query Result
**Validates: Requirements 2.1**

**Property 6: Modification query execution**
*For any* valid INSERT, UPDATE, or DELETE query, the MCP Server should execute it and return the correct number of affected rows
**Validates: Requirements 2.2**

**Property 7: SQL syntax error handling**
*For any* query containing syntax errors, the MCP Server should return the MySQL error message to the client
**Validates: Requirements 2.3**

**Property 8: Concurrent query independence**
*For any* set of concurrent queries, the MCP Server should handle them independently without result interference
**Validates: Requirements 2.5**

**Property 9: Result streaming for large datasets**
*For any* query with large result sets, the MCP Server should stream Query Result rows incrementally to the client
**Validates: Requirements 3.1**

**Property 10: Streaming format consistency**
*For any* streaming operation, the MCP Server should maintain consistent data formatting across all result chunks
**Validates: Requirements 3.2**

**Property 11: Stream completion signaling**
*For any* completed streaming operation, the MCP Server should send a completion signal to indicate the end of the result set
**Validates: Requirements 3.5**

**Property 12: MCP protocol compliance**
*For any* query result, the MCP Server should format data according to MCP protocol specifications
**Validates: Requirements 4.1**

**Property 13: Data type conversion**
*For any* MySQL data type, the MCP Server should convert it to an appropriate JSON-compatible format
**Validates: Requirements 4.2**

**Property 14: Metadata inclusion**
*For any* query result, the MCP Server should include column names, types, and other relevant schema information
**Validates: Requirements 4.3**

**Property 15: Serialization round-trip**
*For any* query result, serializing then deserializing should produce an equivalent result
**Validates: Requirements 4.4**

**Property 16: NULL value consistency**
*For any* query result containing NULL values, the MCP Server should represent them consistently in the JSON response format
**Validates: Requirements 4.5**

**Property 17: Connection error handling**
*For any* database connection error, the MCP Server should log error details and return a user-friendly error message
**Validates: Requirements 5.1**

**Property 18: Query error capture**
*For any* failed query execution, the MCP Server should capture the MySQL error code and message for client response
**Validates: Requirements 5.2**

**Property 19: Error logging safety**
*For any* unexpected error, the MCP Server should log stack traces while returning safe error messages to clients
**Validates: Requirements 5.4**

**Property 20: Connection pool initialization**
*For any* server initialization, the MCP Server should create a Connection Pool with the specified configurable size limits
**Validates: Requirements 6.1**

**Property 21: Connection reuse**
*For any* connection request, the MCP Server should reuse existing connections from the Connection Pool when available
**Validates: Requirements 6.2**

**Property 22: Connection cleanup on shutdown**
*For any* server shutdown, the MCP Server should properly close all database connections in the Connection Pool
**Validates: Requirements 6.4**

**Property 23: Pool limit handling**
*For any* situation where connection pool limits are reached, the MCP Server should queue requests or return appropriate busy signals
**Validates: Requirements 6.5**

## Error Handling

The system implements comprehensive error handling across all layers:

### Connection Errors
- **Database Unavailable**: Return connection timeout errors with retry suggestions
- **Invalid Credentials**: Return authentication failure messages without exposing sensitive details
- **Network Issues**: Implement exponential backoff for transient network failures
- **Pool Exhaustion**: Queue requests with configurable timeout or return busy status

### Query Errors
- **SQL Syntax Errors**: Pass through MySQL error messages with line/column information
- **Permission Errors**: Return access denied messages with affected table/operation details
- **Data Type Errors**: Provide clear conversion error messages with expected vs actual types
- **Constraint Violations**: Return constraint violation details for debugging

### Streaming Errors
- **Connection Interruption**: Clean up partial streams and notify clients of interruption
- **Memory Pressure**: Implement backpressure to prevent memory exhaustion
- **Serialization Failures**: Return specific serialization error details
- **Client Disconnection**: Gracefully terminate streaming operations

### System Errors
- **Resource Exhaustion**: Implement circuit breakers and graceful degradation
- **Configuration Errors**: Validate configuration at startup with clear error messages
- **Dependency Failures**: Provide fallback mechanisms where possible

## Testing Strategy

The testing approach combines unit testing and property-based testing to ensure comprehensive coverage:

### Unit Testing
- Test specific examples of query execution with known inputs and expected outputs
- Test error conditions with invalid inputs (malformed SQL, invalid connections)
- Test edge cases like empty result sets, single-row results, and large datasets
- Test integration points between MCP protocol handling and database operations
- Test connection pool behavior under various load conditions

### Property-Based Testing
- Use `proptest` crate for generating random test inputs
- Configure each property-based test to run a minimum of 100 iterations
- Test universal properties that should hold across all valid inputs
- Each property-based test will be tagged with comments referencing the design document properties
- Tag format: **Feature: mysql-mcp-server, Property {number}: {property_text}**

**Property-Based Testing Framework**: `proptest` crate for Rust
**Test Configuration**: Minimum 100 iterations per property test
**Test Tagging**: Each property test must reference its corresponding design document property

### Testing Dependencies
- `proptest`: For property-based testing framework
- `tokio-test`: For async testing utilities  
- `sqlx-test`: For database testing with test containers
- `serde_json`: For JSON serialization/deserialization testing
- `mockall`: For mocking external dependencies in unit tests