# Integration Tests

This directory contains integration tests for the MySQL MCP Server that test end-to-end functionality with a real MySQL database.

## Running Integration Tests

### Prerequisites

1. **MySQL Database**: You need access to a MySQL database for testing
2. **Test Database URL**: Set the `TEST_DATABASE_URL` environment variable

### Setup

1. Create a test database (or use an existing one that can be safely modified):
   ```sql
   CREATE DATABASE mcp_test;
   ```

2. Set the test database URL environment variable:
   ```bash
   export TEST_DATABASE_URL="mysql://username:password@localhost:3306/mcp_test"
   ```

3. Run the integration tests:
   ```bash
   cargo test --test integration_tests
   ```

### Test Coverage

The integration tests cover:

1. **End-to-End Query Execution**: Tests SELECT, INSERT, UPDATE, and DELETE queries through the MCP protocol
2. **Streaming Functionality**: Tests streaming of large result sets with real database data
3. **MCP Protocol Compliance**: Tests MCP protocol initialization, tool listing, and tool execution
4. **Concurrent Query Execution**: Tests multiple simultaneous queries to verify independence
5. **Error Handling**: Tests various error conditions including SQL syntax errors and non-existent tables

### Test Database Safety

- Tests create and drop a `test_users` table
- Tests clean up after themselves by dropping created tables
- Use a dedicated test database to avoid affecting production data

### Skipping Tests

If `TEST_DATABASE_URL` is not set, all integration tests will be skipped with informational messages. This allows the test suite to run in CI/CD environments without requiring a database.

### Example Output

```
running 5 tests
test test_concurrent_query_execution ... ok
test test_streaming_functionality_with_real_database ... ok
test test_mcp_protocol_compliance ... ok
test test_end_to_end_query_execution_through_mcp ... ok
test test_error_handling_integration ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Troubleshooting

- **Connection Errors**: Verify your MySQL server is running and the connection URL is correct
- **Permission Errors**: Ensure the database user has CREATE, DROP, INSERT, UPDATE, DELETE, and SELECT permissions
- **Timeout Issues**: Integration tests include timeouts to prevent hanging; increase timeout values if needed for slow databases