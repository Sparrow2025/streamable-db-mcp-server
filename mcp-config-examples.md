# MCP Configuration Examples

This document provides various examples of how to configure MCP clients to use the **Streamable HTTP MySQL MCP Server**.

**Important**: This is an HTTP-based MCP server, not a stdio-based server. It runs as an HTTP service that MCP clients connect to.

## Server Setup

First, start the MySQL MCP Server:

```bash
# Development
cargo run

# Production  
cargo run --release

# Custom port
PORT=9090 cargo run --release
```

The server will be available at:
- MCP endpoint: `http://localhost:8080/mcp`
- Streaming endpoint: `http://localhost:8080/stream/query`

## Kiro IDE Configuration

### Basic HTTP Configuration

Create or edit `.kiro/settings/mcp.json`:

```json
{
  "mcpServers": {
    "mysql-server": {
      "url": "http://localhost:8080/mcp",
      "disabled": false,
      "autoApprove": ["test_connection"，"list_databases", "list_tables","describe_table","list_columns","execute_query"]
    }
  }
}
```

### Production Configuration

```json
{
  "mcpServers": {
    "mysql-production": {
      "url": "http://production-server:8080/mcp",
      "disabled": false,
      "autoApprove": ["test_connection"]
    }
  }
}
```

### Custom Port Configuration

If your server runs on a different port:

```json
{
  "mcpServers": {
    "mysql-custom": {
      "url": "http://localhost:9090/mcp",
      "disabled": false,
      "autoApprove": ["test_connection", "execute_query"]
    }
  }
}
```

### Configuration with Headers (for authentication)

If you need to add authentication headers:

```json
{
  "mcpServers": {
    "mysql-secure": {
      "url": "https://secure-mysql-server.com/mcp",
      "headers": {
        "Authorization": "Bearer ${SECRET_TOKEN}",
        "X-API-Key": "${API_KEY}"
      },
      "disabled": false,
      "autoApprove": ["test_connection"]
    }
  }
}
```

**Note**: The `${SECRET_TOKEN}` and `${API_KEY}` will be replaced with actual environment variable values by Kiro IDE.

### Multiple Environment Configuration

```json
{
  "mcpServers": {
    "mysql-dev": {
      "url": "http://localhost:8080/mcp",
      "disabled": false,
      "autoApprove": ["test_connection", "execute_query"]
    },
    "mysql-staging": {
      "url": "http://staging-db:8080/mcp",
      "disabled": false,
      "autoApprove": ["test_connection"]
    },
    "mysql-prod": {
      "url": "http://prod-db:8080/mcp",
      "headers": {
        "Authorization": "Bearer ${PROD_TOKEN}"
      },
      "disabled": false,
      "autoApprove": []
    }
  }
}
```

## Claude Desktop Configuration

### macOS Configuration

File: `~/Library/Application Support/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "mysql": {
      "command": "cargo",
      "args": ["run", "--release"],
      "cwd": "/Users/username/projects/mysql-mcp-server"
    }
  }
}
```

### Windows Configuration

File: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "mysql": {
      "command": "cargo.exe",
      "args": ["run", "--release"],
      "cwd": "C:\\Projects\\mysql-mcp-server"
    }
  }
}
```

### Using Pre-built Binary

```json
{
  "mcpServers": {
    "mysql": {
      "command": "/path/to/mysql-mcp-server",
      "args": [],
      "cwd": "/path/to/config/directory"
    }
  }
}
```

## Advanced Configuration Examples

### HTTPS Configuration

For production deployments with HTTPS:

```json
{
  "mcpServers": {
    "mysql-https": {
      "url": "https://mysql-mcp.yourdomain.com/mcp",
      "headers": {
        "Authorization": "Bearer ${SECRET_TOKEN}"
      },
      "disabled": false,
      "autoApprove": ["test_connection"]
    }
  }
}
```

### Load Balancer Configuration

If using a load balancer:

```json
{
  "mcpServers": {
    "mysql-lb": {
      "url": "http://mysql-lb.internal:8080/mcp",
      "headers": {
        "X-Forwarded-For": "client-ip",
        "X-Service-Name": "kiro-ide"
      },
      "disabled": false,
      "autoApprove": ["test_connection"]
    }
  }
}
```

## Docker Configuration

If you're running the MySQL MCP Server in Docker:

### Dockerfile Example

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/mysql-mcp-server /usr/local/bin/
COPY config.toml /etc/mysql-mcp-server/
WORKDIR /etc/mysql-mcp-server
EXPOSE 8080
CMD ["mysql-mcp-server"]
```

### MCP Configuration for Docker

When the server runs in Docker, you still connect via HTTP:

```json
{
  "mcpServers": {
    "mysql-docker": {
      "url": "http://localhost:8080/mcp",
      "disabled": false,
      "autoApprove": ["test_connection"]
    }
  }
}
```

Or if Docker is on a different host:

```json
{
  "mcpServers": {
    "mysql-docker-remote": {
      "url": "http://docker-host:8080/mcp",
      "disabled": false,
      "autoApprove": ["test_connection"]
    }
  }
}
```

## Multiple Database Configurations

You can configure multiple MySQL servers for different databases by running multiple server instances on different ports:

```json
{
  "mcpServers": {
    "mysql-production": {
      "url": "http://prod-mysql:8080/mcp",
      "headers": {
        "Authorization": "Bearer ${PROD_TOKEN}"
      },
      "disabled": false,
      "autoApprove": ["test_connection"]
    },
    "mysql-staging": {
      "url": "http://staging-mysql:8081/mcp",
      "headers": {
        "Authorization": "Bearer ${STAGING_TOKEN}"
      },
      "disabled": false,
      "autoApprove": ["execute_query", "test_connection"]
    },
    "mysql-development": {
      "url": "http://localhost:8082/mcp",
      "disabled": false,
      "autoApprove": ["execute_query", "test_connection", "streaming_query"]
    }
  }
}
```

## Troubleshooting

### Common Issues

1. **Server not starting**: Check that the `cwd` path is correct and contains `config.toml`
2. **Permission denied**: Ensure the binary has execute permissions
3. **Connection refused**: Verify MySQL server is running and accessible
4. **Tool not found**: Check that the server is properly registered in MCP config

### Debug Configuration

For troubleshooting, use this configuration with detailed logging:

```json
{
  "mcpServers": {
    "mysql-debug": {
      "command": "cargo",
      "args": ["run"],
      "cwd": "/path/to/mysql-mcp-server",
      "env": {
        "RUST_LOG": "debug",
        "LOG_LEVEL": "debug"
      },
      "disabled": false,
      "autoApprove": []
    }
  }
}
```

### Testing Configuration

To test your configuration:

1. Start the MCP client (Kiro IDE, Claude Desktop, etc.)
2. Look for the MySQL server in the available tools
3. Try the `test_connection` tool first
4. Execute a simple query like `SELECT 1`

## Security Considerations

1. **Credentials**: Never put database passwords directly in MCP config files
2. **Auto-approve**: Be cautious with `autoApprove` - only include safe, read-only operations
3. **Network**: Ensure the MySQL server is properly secured
4. **Logging**: Be careful not to log sensitive data in debug mode