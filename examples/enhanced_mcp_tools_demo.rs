//! Enhanced MCP Tools Demo
//! 
//! This example demonstrates the enhanced MCP tools for multi-environment support.
//! It shows how to use the new environment-aware tools and multi-environment capabilities.

use mysql_mcp_server::{Config, McpServer, Result};
use serde_json::json;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("Enhanced MCP Tools Demo");
    println!("======================");

    // Create a sample multi-environment configuration
    let config = create_sample_multi_env_config();
    
    // Create MCP server with multi-environment support
    match McpServer::with_multi_environment(config).await {
        Ok(server) => {
            println!("✓ Multi-environment MCP server created successfully");
            
            // Demonstrate enhanced MCP tools
            demonstrate_enhanced_tools(&server).await?;
        }
        Err(e) => {
            println!("✗ Failed to create multi-environment server: {}", e);
            println!("This is expected if you don't have multiple MySQL instances running");
            println!("The demo shows the tool definitions and structure anyway.");
            
            // Show tool definitions even if we can't connect
            demonstrate_tool_definitions();
        }
    }

    Ok(())
}

/// Create a sample multi-environment configuration for demonstration
fn create_sample_multi_env_config() -> Config {
    use mysql_mcp_server::{ServerConfig, DatabaseConfig, EnvironmentConfig, PoolConfig, McpConfig};
    
    let mut environments = HashMap::new();
    
    // Development environment
    environments.insert("dev".to_string(), EnvironmentConfig {
        name: "dev".to_string(),
        description: Some("Development environment".to_string()),
        database: DatabaseConfig {
            host: "localhost".to_string(),
            port: 3306,
            username: "dev_user".to_string(),
            password: "dev_password".to_string(),
            database: "dev_db".to_string(),
            connection_timeout: 30,
            max_connections: 10,
        },
        connection_pool: PoolConfig {
            max_connections: 5,
            min_connections: 1,
            connection_timeout: 30,
            idle_timeout: 600,
        },
        enabled: true,
    });
    
    // UAT environment
    environments.insert("uat".to_string(), EnvironmentConfig {
        name: "uat".to_string(),
        description: Some("User Acceptance Testing environment".to_string()),
        database: DatabaseConfig {
            host: "localhost".to_string(),
            port: 3307, // Different port for demo
            username: "uat_user".to_string(),
            password: "uat_password".to_string(),
            database: "uat_db".to_string(),
            connection_timeout: 30,
            max_connections: 10,
        },
        connection_pool: PoolConfig {
            max_connections: 5,
            min_connections: 1,
            connection_timeout: 30,
            idle_timeout: 600,
        },
        enabled: true,
    });

    Config {
        server: ServerConfig {
            port: 8080,
            log_level: "info".to_string(),
        },
        database: None, // No legacy database config
        environments: Some(environments),
        default_environment: Some("dev".to_string()),
        mcp: McpConfig {
            protocol_version: "2024-11-05".to_string(),
            server_name: "enhanced-mysql-mcp-server".to_string(),
            server_version: "0.1.0".to_string(),
        },
    }
}

/// Demonstrate the enhanced MCP tools functionality
async fn demonstrate_enhanced_tools(server: &McpServer) -> Result<()> {
    println!("\n📋 Available Enhanced MCP Tools:");
    println!("================================");
    
    // Get list of tools
    let tools_response = server.handle_list_tools().await?;
    if let Some(tools) = tools_response.get("tools").and_then(|v| v.as_array()) {
        for (i, tool) in tools.iter().enumerate() {
            if let Some(name) = tool.get("name").and_then(|v| v.as_str()) {
                if let Some(description) = tool.get("description").and_then(|v| v.as_str()) {
                    println!("{}. {} - {}", i + 1, name, description);
                }
            }
        }
    }

    println!("\n🔧 Demonstrating Enhanced Tools:");
    println!("===============================");

    // Demonstrate list_environments tool
    println!("\n1. Listing environments:");
    match server.handle_call_tool(Some(json!({
        "name": "list_environments",
        "arguments": {}
    }))).await {
        Ok(result) => {
            println!("✓ Environments listed successfully");
            if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
                if let Some(text) = content.get(0).and_then(|v| v.get("text")).and_then(|v| v.as_str()) {
                    println!("Response: {}", text);
                }
            }
        }
        Err(e) => println!("✗ Failed to list environments: {}", e),
    }

    // Demonstrate execute_query_env tool
    println!("\n2. Executing query against specific environment:");
    match server.handle_call_tool(Some(json!({
        "name": "execute_query_env",
        "arguments": {
            "sql": "SELECT 'Hello from Dev!' as message",
            "environment": "dev"
        }
    }))).await {
        Ok(result) => {
            println!("✓ Query executed successfully against dev environment");
            if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
                if let Some(text) = content.get(0).and_then(|v| v.get("text")).and_then(|v| v.as_str()) {
                    println!("Response: {}", text);
                }
            }
        }
        Err(e) => println!("✗ Failed to execute query: {}", e),
    }

    // Demonstrate execute_query_multi_env tool
    println!("\n3. Executing query against multiple environments:");
    match server.handle_call_tool(Some(json!({
        "name": "execute_query_multi_env",
        "arguments": {
            "sql": "SELECT DATABASE() as current_database",
            "environments": ["dev", "uat"],
            "compare_results": true
        }
    }))).await {
        Ok(result) => {
            println!("✓ Multi-environment query executed successfully");
            if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
                if let Some(text) = content.get(0).and_then(|v| v.get("text")).and_then(|v| v.as_str()) {
                    println!("Response: {}", text);
                }
            }
        }
        Err(e) => println!("✗ Failed to execute multi-environment query: {}", e),
    }

    // Demonstrate health_check_env tool
    println!("\n4. Checking environment health:");
    match server.handle_call_tool(Some(json!({
        "name": "health_check_env",
        "arguments": {}
    }))).await {
        Ok(result) => {
            println!("✓ Health check completed successfully");
            if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
                if let Some(text) = content.get(0).and_then(|v| v.get("text")).and_then(|v| v.as_str()) {
                    println!("Response: {}", text);
                }
            }
        }
        Err(e) => println!("✗ Failed to check health: {}", e),
    }

    Ok(())
}

/// Demonstrate tool definitions even when we can't connect to databases
fn demonstrate_tool_definitions() {
    println!("\n📋 Enhanced MCP Tool Definitions:");
    println!("=================================");
    
    let enhanced_tools = [
        ("execute_query_env", "Execute read-only SQL queries against a specific environment"),
        ("execute_query_multi_env", "Execute the same query against multiple environments simultaneously"),
        ("list_environments", "List all configured database environments with their status"),
        ("list_databases_env", "List all databases in a specific environment"),
        ("list_databases_all_env", "List all databases across all enabled environments"),
        ("list_tables_env", "List all tables in a specific database and environment"),
        ("describe_table_env", "Get detailed table structure information from a specific environment"),
        ("compare_schema", "Compare database schema across multiple environments"),
        ("health_check_env", "Check the health status of database connections"),
        ("test_connection_env", "Test database connection for a specific environment"),
    ];

    for (i, (name, description)) in enhanced_tools.iter().enumerate() {
        println!("{}. {} - {}", i + 1, name, description);
    }

    println!("\n💡 Key Features:");
    println!("================");
    println!("• Environment-aware query execution");
    println!("• Multi-environment query comparison");
    println!("• Schema comparison across environments");
    println!("• Health monitoring for all environments");
    println!("• Backward compatibility with existing tools");
    println!("• Secure credential isolation per environment");
    
    println!("\n📖 Usage Examples:");
    println!("==================");
    println!("1. Query specific environment:");
    println!("   execute_query_env(sql='SELECT * FROM users', environment='dev')");
    
    println!("\n2. Compare data across environments:");
    println!("   execute_query_multi_env(sql='SELECT COUNT(*) FROM orders', environments=['dev', 'uat'], compare_results=true)");
    
    println!("\n3. Compare schema differences:");
    println!("   compare_schema(environments=['dev', 'prod'], database='myapp')");
    
    println!("\n4. Monitor environment health:");
    println!("   health_check_env() // Check all environments");
    println!("   health_check_env(environment='prod') // Check specific environment");
}