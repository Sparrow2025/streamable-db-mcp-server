# MySQL MCP Server

A Streamable MySQL MCP (Model Context Protocol) server implementation in Rust that provides database connectivity and query execution capabilities.

## Configuration

The server supports configuration through TOML files with fallback to environment variables. The new configuration format separates database connection details into individual fields for better clarity and security.

### Configuration File

Create a `config.toml` file in the project root:

```toml
[server]
# Server listening port
port = 8080
# Log level: trace, debug, info, warn, error
log_level = "info"

[database]
# Database connection details
host = "localhost"          # Database host
port = 3306                 # Database port (optional, default: 3306)
username = "root"           # Database username
password = "password"       # Database password
database = "myapp"          # Database name
# Connection timeout in seconds (optional, default: 30)
connection_timeout = 30
# Maximum number of connections in the pool (optional, default: 10)
max_connections = 10

[mcp]
# MCP protocol version
protocol_version = "2024-11-05"
# Server identification
server_name = "mysql-mcp-server"
server_version = "0.1.0"
```

### Configuration Options

#### Database Section
- `host`: MySQL server hostname or IP address
- `port`: MySQL server port (optional, default: 3306)
- `username`: Database username
- `password`: Database password
- `database`: Database name to connect to
- `connection_timeout`: Connection timeout in seconds (optional, default: 30)
- `max_connections`: Maximum connections in pool (optional, default: 10)

#### Server Section
- `port`: HTTP server listening port (default: 8080)
- `log_level`: Logging level (trace, debug, info, warn, error)

#### MCP Section
- `protocol_version`: MCP protocol version
- `server_name`: Server identification name
- `server_version`: Server version string

### Configuration File Locations

The server will look for configuration files in the following order:
1. `config.toml` (current directory)
2. `./config.toml`
3. `config/config.toml`

### Environment Variables (Fallback)

If no configuration file is found, the server will use environment variables:

```bash
# Individual database components (preferred)
export DB_HOST="localhost"
export DB_PORT=3306
export DB_USERNAME="root"
export DB_PASSWORD="password"
export DB_DATABASE="myapp"

# Or use DATABASE_URL (legacy support)
export DATABASE_URL="mysql://username:password@localhost:3306/database"

# Server configuration
export PORT=8080
export LOG_LEVEL=info
```

## Quick Start

### Automated Setup (Recommended)

Run the setup script to automatically configure everything:

```bash
./setup-mcp.sh
```

This script will:
- Build the project
- Create `config.toml` from the example
- Generate MCP client configuration
- Provide next steps

### Manual Setup

1. **Copy the example configuration:**
   ```bash
   cp config.example.toml config.toml
   ```

2. **Edit the configuration:**
   Update the database connection details in `config.toml`:
   ```toml
   [database]
   host = "your-mysql-host"
   port = 3306
   username = "your-username"
   password = "your-password"
   database = "your-database-name"
   ```

3. **Build and run the server:**
   ```bash
   cargo build --release
   cargo run
   ```

4. **Test the configuration:**
   ```bash
   ./test-mcp-connection.sh
   ```

5. **Test HTTP endpoints (after starting server):**
   ```bash
   # In one terminal, start the server:
   cargo run --release
   
   # In another terminal, test the endpoints:
   ./test-http-endpoints.sh
   ```

5. **Configure your MCP client** (see [MCP Client Configuration](#mcp-client-configuration) section)

## Development

### Running Tests

```bash
# Run unit tests (no database required)
cargo test --lib

# Run integration tests (requires TEST_DATABASE_URL)
export TEST_DATABASE_URL="mysql://root:password@localhost:3306/test_db"
cargo test

# Run all tests
cargo test
```

### Database Setup for Testing

For integration tests, you can use Docker to set up a MySQL instance:

```bash
# Start MySQL container
docker run --name mysql-test \
  -e MYSQL_ROOT_PASSWORD=password \
  -e MYSQL_DATABASE=test_db \
  -p 3306:3306 \
  -d mysql:8.0

# Set test database URL
export TEST_DATABASE_URL="mysql://root:password@localhost:3306/test_db"

# Run tests
cargo test
```

## Features

- **Streamable HTTP Transport**: Efficient data transfer using rmcp
- **Read-Only Query Execution**: Support for SELECT, SHOW, DESCRIBE, EXPLAIN operations (write operations blocked for security)
- **Database Exploration**: List databases, tables, and inspect table structures
- **Result Streaming**: Handle large result sets with incremental delivery
- **Security**: Only read-only queries allowed (INSERT, UPDATE, DELETE blocked)
- **Error Handling**: Comprehensive error reporting and logging
- **Configuration Management**: Flexible TOML-based configuration
- **Property-Based Testing**: Robust testing with proptest

## MCP Client Configuration

This is a **Streamable HTTP MCP Server** that runs as an HTTP service. MCP clients connect to it via HTTP endpoints rather than stdio.

### Server Endpoints

Once running, the server provides these HTTP endpoints:

- **MCP Protocol**: `http://localhost:8080/mcp` (JSON-RPC over HTTP)
- **Streaming Queries**: `http://localhost:8080/stream/query` (Server-Sent Events)

### Kiro IDE Configuration

1. **Start the MySQL MCP Server**:
   ```bash
   cargo run --release
   ```
   The server will start on `http://localhost:8080` by default.

2. **Configure Kiro IDE** (`.kiro/settings/mcp.json`):

```json
{
  "mcpServers": {
    "mysql-server": {
      "url": "http://localhost:8080/mcp",
      "disabled": false,
      "autoApprove": ["test_connection"]
    }
  }
}
```

### Claude Desktop Configuration

**Note**: Claude Desktop may not support HTTP-based MCP servers yet. Check the latest Claude Desktop documentation for HTTP MCP support.

If supported, the configuration would be:

```json
{
  "mcpServers": {
    "mysql-server": {
      "url": "http://localhost:8080/mcp"
    }
  }
}
```

### Manual HTTP Testing

You can test the server directly with HTTP requests:

```bash
# Test connection
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
      "name": "test_connection",
      "arguments": {}
    }
  }'

# Execute a query
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0", 
    "id": 2,
    "method": "tools/call",
    "params": {
      "name": "execute_query",
      "arguments": {
        "sql": "SELECT 1 as test"
      }
    }
  }'
```

### Testing the HTTP Server

You can test the server directly before configuring MCP clients:

```bash
# 1. Start the server
cargo run --release

# 2. In another terminal, test the endpoints:

# List available tools
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1, 
    "method": "tools/list"
  }'

# Test database connection
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
      "name": "test_connection",
      "arguments": {}
    }
  }'
```

### Available MCP Tools

Once configured, the following tools will be available in your MCP client:

#### 1. `test_connection`
Test the database connection.

**Parameters**: None

**Example usage**:
```
Test the MySQL database connection
```

#### 2. `execute_query`
Execute read-only SQL queries against the database. Only SELECT, SHOW, DESCRIBE, and EXPLAIN statements are allowed for security reasons.

**Parameters**:
- `sql` (string): The read-only SQL query to execute (SELECT, SHOW, DESCRIBE, EXPLAIN only)
- `parameters` (array, optional): Query parameters for prepared statements
- `stream_results` (boolean, optional): Whether to stream large result sets

**Example usage**:
```
Execute this SQL query: SELECT * FROM users WHERE age > 25
Show me all tables: SHOW TABLES
Describe table structure: DESCRIBE users
```

**Security Note**: Write operations (INSERT, UPDATE, DELETE, DROP, CREATE, ALTER) are blocked for security reasons.

#### 3. `streaming_query`
Execute queries with streaming support for large result sets.

**Parameters**:
- `sql` (string): The SQL query to execute
- `parameters` (array, optional): Query parameters

**Example usage**:
```
Stream results from: SELECT * FROM large_table ORDER BY created_at
```

### Configuration Tips

1. **Auto-approve tools**: Add frequently used tools to `autoApprove` to skip confirmation prompts
2. **Working directory**: Set `cwd` to the directory containing your `config.toml` file
3. **Environment variables**: Use `env` to set database credentials if not using config file
4. **Logging**: Set `RUST_LOG=debug` for detailed logging during development

### Example MCP Session

Once configured, you can interact with your MySQL database through natural language:

```
User: "Show me all users from the database"
Assistant: I'll query the users table for you.
[Executes: SELECT * FROM users]

User: "Create a new user named John with email john@example.com"
Assistant: I'll insert a new user record.
[Executes: INSERT INTO users (name, email) VALUES ('John', 'john@example.com')]

User: "Show me the total count of orders by status"
Assistant: I'll get the order counts grouped by status.
[Executes: SELECT status, COUNT(*) as count FROM orders GROUP BY status]
```

## Security

This MCP server is designed with security in mind:

- **Read-Only Operations**: Only SELECT, SHOW, DESCRIBE, and EXPLAIN queries are allowed
- **Write Operations Blocked**: INSERT, UPDATE, DELETE, DROP, CREATE, ALTER operations are rejected
- **SQL Injection Protection**: All queries are validated before execution
- **Connection Security**: Uses secure database connections with proper authentication

### Configuration Security

⚠️ **Important Security Notes**:

- **Never commit `config.toml`** - it contains sensitive database credentials
- The `.gitignore` file is configured to exclude `config.toml` and other sensitive files
- Use `config.example.toml` as a template for your configuration
- Consider using environment variables for production deployments
- Ensure your database user has minimal required permissions (read-only recommended)

### Allowed SQL Operations

✅ **Permitted**:
- `SELECT` - Query data
- `SHOW` - Display database metadata (tables, databases, etc.)
- `DESCRIBE` / `DESC` - Show table structure
- `EXPLAIN` - Query execution plans

❌ **Blocked**:
- `INSERT`, `UPDATE`, `DELETE` - Data modification
- `CREATE`, `ALTER`, `DROP` - Schema changes
- `TRUNCATE` - Data deletion
- `GRANT`, `REVOKE` - Permission changes
- Any other write operations

## MCP Protocol Compliance

The server implements the Model Context Protocol (MCP) specification and provides tools for:
- Database connection testing
- Read-only SQL query execution
- Database and table exploration
- Result streaming for large datasets
- Error handling and reporting

## License

This project is licensed under the MIT License.