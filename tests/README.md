# Integration Tests

This directory contains integration tests for the MySQL MCP Server that test end-to-end functionality with real MySQL databases.

## Test Files

### `integration_tests.rs`
Basic integration tests for single-database functionality:
- End-to-end query execution through MCP protocol
- Streaming functionality with real database data
- MCP protocol compliance
- Concurrent query execution
- Error handling

### `multi_environment_integration_tests.rs`
Comprehensive integration tests for multi-environment functionality:
- End-to-end multi-environment query execution
- Connection failover and recovery scenarios
- MCP protocol compliance with multiple environments
- Docker deployment with multiple environment configurations
- Performance and concurrent access testing
- Multi-environment streaming functionality

## Running Integration Tests

### Single Database Tests

#### Prerequisites
1. **MySQL Database**: You need access to a MySQL database for testing
2. **Test Database URL**: Set the `TEST_DATABASE_URL` environment variable

#### Setup
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

### Multi-Environment Tests

#### Prerequisites
For comprehensive multi-environment testing, you can configure multiple test databases:

1. **Development Environment Database**:
   ```bash
   export TEST_DEV_DATABASE_URL="mysql://dev_user:dev_password@localhost:3306/dev_test_db"
   ```

2. **Staging Environment Database**:
   ```bash
   export TEST_STAGING_DATABASE_URL="mysql://staging_user:staging_password@localhost:3307/staging_test_db"
   ```

3. **Production Test Environment Database** (optional):
   ```bash
   export TEST_PROD_DATABASE_URL="mysql://prod_user:prod_password@localhost:3308/prod_test_db"
   ```

#### Fallback Configuration
If multi-environment URLs are not available, the tests will fall back to using `TEST_DATABASE_URL` for basic testing.

#### Running Multi-Environment Tests
```bash
# Run all multi-environment integration tests
cargo test --test multi_environment_integration_tests

# Run specific test categories
cargo test --test multi_environment_integration_tests test_end_to_end_multi_environment_query_execution
cargo test --test multi_environment_integration_tests test_connection_failover_and_recovery
cargo test --test multi_environment_integration_tests test_mcp_protocol_compliance_multi_environment
cargo test --test multi_environment_integration_tests test_docker_multi_environment_configuration
```

## Test Coverage

### Single Database Integration Tests
1. **End-to-End Query Execution**: Tests SELECT, INSERT, UPDATE, and DELETE queries through the MCP protocol
2. **Streaming Functionality**: Tests streaming of large result sets with real database data
3. **MCP Protocol Compliance**: Tests MCP protocol initialization, tool listing, and tool execution
4. **Concurrent Query Execution**: Tests multiple simultaneous queries to verify independence
5. **Error Handling**: Tests various error conditions including SQL syntax errors and non-existent tables

### Multi-Environment Integration Tests
1. **End-to-End Multi-Environment Query Execution**:
   - Single environment query execution
   - Multi-environment query execution
   - Environment comparison queries
   - Schema comparison across environments

2. **Connection Failover and Recovery Scenarios**:
   - Graceful startup with partial environment failures
   - Health check functionality
   - Individual environment health checks
   - Connection testing for specific environments
   - Resilience to environment failures during operation

3. **MCP Protocol Compliance with Multiple Environments**:
   - MCP initialize with multi-environment support
   - Multi-environment tools availability
   - Tool execution following MCP protocol
   - Error handling following MCP protocol
   - Parameter validation following MCP protocol

4. **Docker Deployment Configuration Testing**:
   - Docker-style configuration parsing
   - Environment variable configuration parsing
   - Docker Compose service name resolution
   - Configuration validation and error handling

5. **Performance and Concurrent Access**:
   - Concurrent query execution across environments
   - Performance metrics collection
   - Monitoring report generation

6. **Multi-Environment Streaming**:
   - Single environment streaming
   - Multi-environment streaming
   - Streaming error handling

## Test Database Safety

- Tests create and drop temporary tables (e.g., `integration_test_users`, `test_users`)
- Tests clean up after themselves by dropping created tables
- Use dedicated test databases to avoid affecting production data
- All test data is clearly marked with environment-specific identifiers

## Skipping Tests

### Automatic Skipping
- If `TEST_DATABASE_URL` is not set, single database tests will be skipped
- If multi-environment URLs are not configured, multi-environment tests will be skipped
- Tests that require real database connections will skip gracefully with informational messages

### Manual Test Selection
```bash
# Run only tests that don't require real databases
cargo test --test multi_environment_integration_tests test_docker_multi_environment_configuration

# Run with verbose output to see skip messages
cargo test --test integration_tests -- --nocapture
```

## Docker Integration Testing

The multi-environment tests include specific support for Docker-based testing:

### Docker Compose Setup
If you're using the provided Docker Compose configurations:

```bash
# Start multi-environment Docker setup
cd docker
./start-multi-env.sh

# Set environment variables for Docker testing
export TEST_DEV_DATABASE_URL="mysql://dev_user:dev_password@localhost:3306/dev_database"
export TEST_STAGING_DATABASE_URL="mysql://staging_user:staging_password@localhost:3307/staging_database"

# Run tests
cargo test --test multi_environment_integration_tests
```

### Docker Configuration Testing
The tests include specific validation for Docker deployment scenarios:
- Docker service name resolution
- Environment variable configuration
- Docker Compose service integration
- Container networking compatibility

## Example Output

### Successful Multi-Environment Test Run
```
running 6 tests
test test_docker_multi_environment_configuration ... ok
test test_connection_failover_and_recovery ... ok
test test_end_to_end_multi_environment_query_execution ... ok
test test_mcp_protocol_compliance_multi_environment ... ok
test test_multi_environment_performance_and_concurrency ... ok
test test_multi_environment_streaming ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Skipped Tests (No Database Configuration)
```
running 6 tests
Skipping multi-environment integration test - insufficient test database URLs configured
Set TEST_DEV_DATABASE_URL and TEST_STAGING_DATABASE_URL to run this test
test test_end_to_end_multi_environment_query_execution ... ok
...
```

## Troubleshooting

### Connection Errors
- **Single Database**: Verify your MySQL server is running and `TEST_DATABASE_URL` is correct
- **Multi-Environment**: Check that all configured database URLs are accessible
- **Docker**: Ensure Docker containers are running and ports are correctly mapped

### Permission Errors
- Ensure database users have CREATE, DROP, INSERT, UPDATE, DELETE, and SELECT permissions
- For multi-environment testing, verify permissions for all configured databases

### Timeout Issues
- Integration tests include timeouts to prevent hanging
- Increase timeout values in test configuration if needed for slow databases
- Check network connectivity between test runner and database servers

### Docker-Specific Issues
- Verify Docker services are running: `docker-compose ps`
- Check Docker network connectivity: `docker network ls`
- Ensure port mappings are correct in docker-compose.yml files
- Check Docker logs: `docker-compose logs mysql-dev mysql-staging`

### Environment Configuration Issues
- Verify environment variables are set correctly: `env | grep TEST_`
- Check that database URLs follow the correct format: `mysql://user:password@host:port/database`
- Ensure database names and credentials match your test setup

## CI/CD Integration

The integration tests are designed to work in CI/CD environments:

1. **Graceful Skipping**: Tests skip automatically when databases aren't available
2. **Environment Detection**: Tests detect and adapt to available database configurations
3. **Timeout Handling**: Tests include appropriate timeouts to prevent CI/CD pipeline hanging
4. **Clear Error Messages**: Failed tests provide clear, actionable error messages

### GitHub Actions Example
```yaml
- name: Setup Test Databases
  run: |
    # Start test databases
    docker-compose -f docker/docker-compose.multi-env.yml up -d mysql-dev mysql-staging
    
- name: Set Test Environment Variables
  run: |
    echo "TEST_DEV_DATABASE_URL=mysql://dev_user:dev_password@localhost:3306/dev_database" >> $GITHUB_ENV
    echo "TEST_STAGING_DATABASE_URL=mysql://staging_user:staging_password@localhost:3307/staging_database" >> $GITHUB_ENV
    
- name: Run Integration Tests
  run: |
    cargo test --test integration_tests
    cargo test --test multi_environment_integration_tests
```