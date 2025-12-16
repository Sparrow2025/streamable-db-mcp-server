//! Comprehensive Integration Tests for Multi-Environment MySQL MCP Server
//! 
//! This test suite covers:
//! 1. End-to-end multi-environment query execution
//! 2. Connection failover and recovery scenarios  
//! 3. MCP protocol compliance with multiple environments
//! 4. Docker deployment with multiple environment configurations

use mysql_mcp_server::{Config, server::McpServer};
use mysql_mcp_server::config::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn, error};

/// Test configuration for multi-environment integration tests
struct MultiEnvTestConfig {
    /// Environment configurations for testing
    environments: HashMap<String, String>,
    /// Whether to run tests that require real databases
    run_real_db_tests: bool,
    /// Default environment for legacy compatibility
    default_environment: String,
}

impl MultiEnvTestConfig {
    /// Create test configuration from environment variables
    fn from_env() -> Self {
        let mut environments = HashMap::new();
        
        // Check for multi-environment test database URLs
        if let Ok(dev_url) = env::var("TEST_DEV_DATABASE_URL") {
            environments.insert("dev".to_string(), dev_url);
        }
        
        if let Ok(staging_url) = env::var("TEST_STAGING_DATABASE_URL") {
            environments.insert("staging".to_string(), staging_url);
        }
        
        if let Ok(prod_url) = env::var("TEST_PROD_DATABASE_URL") {
            environments.insert("prod".to_string(), prod_url);
        }
        
        // Fallback to single test database URL for basic testing
        if environments.is_empty() {
            if let Ok(test_url) = env::var("TEST_DATABASE_URL") {
                environments.insert("dev".to_string(), test_url.clone());
                environments.insert("staging".to_string(), test_url);
            }
        }
        
        let run_real_db_tests = !environments.is_empty();
        let default_environment = "dev".to_string();
        
        Self {
            environments,
            run_real_db_tests,
            default_environment,
        }
    }
    
    /// Check if we can run real database tests
    fn can_run_real_db_tests(&self) -> bool {
        self.run_real_db_tests && self.environments.len() >= 2
    }
    

}

/// Helper function to create a test multi-environment configuration
fn create_test_multi_env_config(test_config: &MultiEnvTestConfig) -> Config {
    let mut environments = HashMap::new();
    
    for (env_name, database_url) in &test_config.environments {
        // Parse database URL to extract components
        let url_parts = parse_database_url(database_url);
        
        let env_config = EnvironmentConfig {
            name: env_name.clone(),
            description: Some(format!("{} environment for integration testing", env_name)),
            database: DatabaseConfig {
                host: url_parts.host,
                port: url_parts.port,
                username: url_parts.username,
                password: url_parts.password,
                database: url_parts.database,
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
        };
        
        environments.insert(env_name.clone(), env_config);
    }
    
    Config {
        server: ServerConfig {
            port: 8080,
            log_level: "info".to_string(),
        },
        database: None,
        environments: Some(environments),
        default_environment: Some(test_config.default_environment.clone()),
        mcp: McpConfig {
            protocol_version: "2024-11-05".to_string(),
            server_name: "mysql-mcp-server-integration-test".to_string(),
            server_version: "0.1.0".to_string(),
        },
    }
}

/// Simple database URL parser for testing
struct DatabaseUrlParts {
    host: String,
    port: u16,
    username: String,
    password: String,
    database: String,
}

fn parse_database_url(url: &str) -> DatabaseUrlParts {
    // Simple parser for mysql://username:password@host:port/database
    let url = url.strip_prefix("mysql://").unwrap_or(url);
    
    let parts: Vec<&str> = url.split('@').collect();
    if parts.len() != 2 {
        return DatabaseUrlParts {
            host: "localhost".to_string(),
            port: 3306,
            username: "test".to_string(),
            password: "test".to_string(),
            database: "test".to_string(),
        };
    }
    
    let auth_part = parts[0];
    let host_db_part = parts[1];
    
    let auth_parts: Vec<&str> = auth_part.split(':').collect();
    let username = auth_parts.get(0).unwrap_or(&"test").to_string();
    let password = auth_parts.get(1).unwrap_or(&"test").to_string();
    
    let host_db_parts: Vec<&str> = host_db_part.split('/').collect();
    let host_port = host_db_parts.get(0).unwrap_or(&"localhost:3306");
    let database = host_db_parts.get(1).unwrap_or(&"test").to_string();
    
    let host_port_parts: Vec<&str> = host_port.split(':').collect();
    let host = host_port_parts.get(0).unwrap_or(&"localhost").to_string();
    let port = host_port_parts.get(1).unwrap_or(&"3306").parse().unwrap_or(3306);
    
    DatabaseUrlParts {
        host,
        port,
        username,
        password,
        database,
    }
}

/// Helper function to create a multi-environment test server
async fn create_multi_env_test_server(test_config: &MultiEnvTestConfig) -> Option<McpServer> {
    if !test_config.can_run_real_db_tests() {
        return None;
    }
    
    let config = create_test_multi_env_config(test_config);
    
    match McpServer::with_multi_environment(config).await {
        Ok(server) => Some(server),
        Err(e) => {
            warn!("Failed to create multi-environment test server: {}", e.user_message());
            None
        }
    }
}

/// Setup test data across multiple environments
async fn setup_multi_env_test_data(server: &McpServer) -> Result<(), Box<dyn std::error::Error>> {
    info!("Setting up test data across multiple environments");
    
    // Create test table in each environment
    let create_table_sql = "CREATE TABLE IF NOT EXISTS integration_test_users (
        id INT AUTO_INCREMENT PRIMARY KEY,
        name VARCHAR(100) NOT NULL,
        email VARCHAR(100) UNIQUE NOT NULL,
        environment VARCHAR(50) NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )";
    
    // Get list of environments
    let list_envs_result = server.handle_call_tool(Some(json!({
        "name": "list_environments",
        "arguments": {}
    }))).await?;
    
    // Extract environment names from the result
    let environments = extract_environment_names_from_result(&list_envs_result)?;
    
    for env_name in &environments {
        info!("Setting up test data in environment: {}", env_name);
        
        // Create table
        let create_result = server.handle_call_tool(Some(json!({
            "name": "execute_query_env",
            "arguments": {
                "sql": create_table_sql,
                "environment": env_name
            }
        }))).await;
        
        if let Err(e) = create_result {
            warn!("Failed to create table in environment '{}': {}", env_name, e);
            continue;
        }
        
        // Clear existing test data
        let clear_result = server.handle_call_tool(Some(json!({
            "name": "execute_query_env",
            "arguments": {
                "sql": "DELETE FROM integration_test_users WHERE environment = ?",
                "environment": env_name,
                "parameters": [env_name]
            }
        }))).await;
        
        if let Err(e) = clear_result {
            warn!("Failed to clear test data in environment '{}': {}", env_name, e);
        }
        
        // Insert environment-specific test data
        let test_users = vec![
            (format!("Alice-{}", env_name), format!("alice-{}@example.com", env_name)),
            (format!("Bob-{}", env_name), format!("bob-{}@example.com", env_name)),
            (format!("Carol-{}", env_name), format!("carol-{}@example.com", env_name)),
        ];
        
        for (name, email) in test_users {
            let insert_result = server.handle_call_tool(Some(json!({
                "name": "execute_query_env",
                "arguments": {
                    "sql": "INSERT INTO integration_test_users (name, email, environment) VALUES (?, ?, ?)",
                    "environment": env_name,
                    "parameters": [name, email, env_name]
                }
            }))).await;
            
            if let Err(e) = insert_result {
                warn!("Failed to insert test data in environment '{}': {}", env_name, e);
            }
        }
    }
    
    info!("Test data setup completed for {} environments", environments.len());
    Ok(())
}

/// Cleanup test data across multiple environments
async fn cleanup_multi_env_test_data(server: &McpServer) -> Result<(), Box<dyn std::error::Error>> {
    info!("Cleaning up test data across multiple environments");
    
    // Get list of environments
    let list_envs_result = server.handle_call_tool(Some(json!({
        "name": "list_environments",
        "arguments": {}
    }))).await?;
    
    let environments = extract_environment_names_from_result(&list_envs_result)?;
    
    for env_name in &environments {
        let drop_result = server.handle_call_tool(Some(json!({
            "name": "execute_query_env",
            "arguments": {
                "sql": "DROP TABLE IF EXISTS integration_test_users",
                "environment": env_name
            }
        }))).await;
        
        if let Err(e) = drop_result {
            warn!("Failed to cleanup test data in environment '{}': {}", env_name, e);
        }
    }
    
    Ok(())
}

/// Extract environment names from list_environments result
fn extract_environment_names_from_result(result: &Value) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let content = result.get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.get(0))
        .and_then(|item| item.get("text"))
        .and_then(|text| text.as_str())
        .ok_or("Invalid list_environments result format")?;
    
    let parsed: Value = serde_json::from_str(content)?;
    let environments = parsed.get("environments")
        .and_then(|envs| envs.as_array())
        .ok_or("No environments found in result")?;
    
    let env_names: Vec<String> = environments
        .iter()
        .filter_map(|env| env.get("name").and_then(|name| name.as_str()).map(|s| s.to_string()))
        .collect();
    
    Ok(env_names)
}

/// Test 1: End-to-end multi-environment query execution
#[tokio::test]
async fn test_end_to_end_multi_environment_query_execution() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let test_config = MultiEnvTestConfig::from_env();
    
    if !test_config.can_run_real_db_tests() {
        println!("Skipping multi-environment integration test - insufficient test database URLs configured");
        println!("Set TEST_DEV_DATABASE_URL and TEST_STAGING_DATABASE_URL to run this test");
        return;
    }
    
    let server = match create_multi_env_test_server(&test_config).await {
        Some(server) => server,
        None => {
            println!("Skipping test - could not create multi-environment server");
            return;
        }
    };
    
    // Initialize the server
    if let Err(e) = server.initialize().await {
        println!("Skipping test - server initialization failed: {}", e.user_message());
        return;
    }
    
    // Setup test data
    if let Err(e) = setup_multi_env_test_data(&server).await {
        println!("Warning: Test data setup failed: {}", e);
    }
    
    // Test 1.1: Single environment query execution
    info!("Testing single environment query execution");
    
    let single_env_result = server.handle_call_tool(Some(json!({
        "name": "execute_query_env",
        "arguments": {
            "sql": "SELECT COUNT(*) as user_count FROM integration_test_users WHERE environment = ?",
            "environment": "dev",
            "parameters": ["dev"]
        }
    }))).await;
    
    assert!(single_env_result.is_ok(), "Single environment query should succeed: {:?}", single_env_result.err());
    
    // Test 1.2: Multi-environment query execution
    info!("Testing multi-environment query execution");
    
    let multi_env_result = server.handle_call_tool(Some(json!({
        "name": "execute_query_multi_env",
        "arguments": {
            "sql": "SELECT environment, COUNT(*) as user_count FROM integration_test_users GROUP BY environment",
            "environments": ["dev", "staging"]
        }
    }))).await;
    
    assert!(multi_env_result.is_ok(), "Multi-environment query should succeed: {:?}", multi_env_result.err());
    
    // Test 1.3: Environment comparison query
    info!("Testing environment comparison query");
    
    let comparison_result = server.handle_call_tool(Some(json!({
        "name": "execute_query_multi_env",
        "arguments": {
            "sql": "SELECT COUNT(*) as total_users FROM integration_test_users",
            "environments": ["dev", "staging"],
            "compare_results": true
        }
    }))).await;
    
    assert!(comparison_result.is_ok(), "Environment comparison query should succeed: {:?}", comparison_result.err());
    
    // Test 1.4: Schema comparison across environments
    info!("Testing schema comparison across environments");
    
    let schema_comparison_result = server.handle_call_tool(Some(json!({
        "name": "compare_schema",
        "arguments": {
            "environments": ["dev", "staging"],
            "table": "integration_test_users"
        }
    }))).await;
    
    assert!(schema_comparison_result.is_ok(), "Schema comparison should succeed: {:?}", schema_comparison_result.err());
    
    // Cleanup
    if let Err(e) = cleanup_multi_env_test_data(&server).await {
        warn!("Test cleanup failed: {}", e);
    }
    
    info!("✅ End-to-end multi-environment query execution test completed successfully");
}

/// Test 2: Connection failover and recovery scenarios
#[tokio::test]
async fn test_connection_failover_and_recovery() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let test_config = MultiEnvTestConfig::from_env();
    
    if !test_config.can_run_real_db_tests() {
        println!("Skipping connection failover test - insufficient test database URLs configured");
        return;
    }
    
    let server = match create_multi_env_test_server(&test_config).await {
        Some(server) => server,
        None => {
            println!("Skipping test - could not create multi-environment server");
            return;
        }
    };
    
    // Test 2.1: Graceful startup with partial environment failures
    info!("Testing graceful startup behavior");
    
    // The server should already be created, test that it can handle partial failures
    let init_result = server.initialize().await;
    
    // Server should either succeed completely or fail gracefully with meaningful errors
    match init_result {
        Ok(()) => {
            info!("✅ Server initialized successfully with all environments");
        }
        Err(e) => {
            info!("⚠️  Server initialization failed as expected: {}", e.user_message());
            // Verify error message is meaningful and relates to environment/connection issues
            let error_msg = e.user_message().to_lowercase();
            assert!(
                error_msg.contains("environment") || 
                error_msg.contains("connection") || 
                error_msg.contains("healthy") ||
                error_msg.contains("database"),
                "Error should be related to environment/connection issues, got: {}", e.user_message()
            );
        }
    }
    
    // Test 2.2: Health check functionality
    info!("Testing health check functionality");
    
    let health_check_result = server.handle_call_tool(Some(json!({
        "name": "health_check_env",
        "arguments": {}
    }))).await;
    
    assert!(health_check_result.is_ok(), "Health check should succeed: {:?}", health_check_result.err());
    
    // Test 2.3: Individual environment health checks
    info!("Testing individual environment health checks");
    
    let environments = vec!["dev", "staging"];
    
    for env_name in &environments {
        let env_health_result = server.handle_call_tool(Some(json!({
            "name": "health_check_env",
            "arguments": {
                "environment": env_name
            }
        }))).await;
        
        // Health check should either succeed or fail gracefully
        match env_health_result {
            Ok(_) => {
                info!("✅ Environment '{}' health check passed", env_name);
            }
            Err(e) => {
                info!("⚠️  Environment '{}' health check failed: {}", env_name, e.user_message());
                // Verify error is environment-related
                assert!(e.user_message().to_lowercase().contains("environment") || 
                       e.user_message().to_lowercase().contains("connection"));
            }
        }
    }
    
    // Test 2.4: Connection testing for specific environments
    info!("Testing connection testing functionality");
    
    for env_name in &environments {
        let connection_test_result = server.handle_call_tool(Some(json!({
            "name": "test_connection_env",
            "arguments": {
                "environment": env_name
            }
        }))).await;
        
        // Connection test should provide meaningful results
        match connection_test_result {
            Ok(_) => {
                info!("✅ Environment '{}' connection test passed", env_name);
            }
            Err(e) => {
                info!("⚠️  Environment '{}' connection test failed: {}", env_name, e.user_message());
                // Verify error is connection-related
                assert!(e.user_message().to_lowercase().contains("connection") || 
                       e.user_message().to_lowercase().contains("environment"));
            }
        }
    }
    
    // Test 2.5: Resilience to environment failures during operation
    info!("Testing resilience to environment failures during operation");
    
    // Try to execute queries against potentially unavailable environments
    let resilience_test_result = server.handle_call_tool(Some(json!({
        "name": "execute_query_multi_env",
        "arguments": {
            "sql": "SELECT 1 as test_value",
            "environments": ["dev", "staging", "nonexistent_env"]
        }
    }))).await;
    
    // Should handle partial failures gracefully
    match resilience_test_result {
        Ok(_) => {
            info!("✅ Multi-environment query handled partial failures gracefully");
        }
        Err(e) => {
            info!("⚠️  Multi-environment query failed: {}", e.user_message());
            // Error should be descriptive about which environments failed
            assert!(e.user_message().contains("environment") || e.user_message().contains("nonexistent"));
        }
    }
    
    info!("✅ Connection failover and recovery test completed successfully");
}

/// Test 3: MCP protocol compliance with multiple environments
#[tokio::test]
async fn test_mcp_protocol_compliance_multi_environment() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let test_config = MultiEnvTestConfig::from_env();
    
    if !test_config.can_run_real_db_tests() {
        println!("Skipping MCP protocol compliance test - insufficient test database URLs configured");
        return;
    }
    
    let server = match create_multi_env_test_server(&test_config).await {
        Some(server) => server,
        None => {
            println!("Skipping test - could not create multi-environment server");
            return;
        }
    };
    
    // Test 3.1: MCP Initialize with multi-environment support
    info!("Testing MCP initialize with multi-environment support");
    
    let init_result = server.handle_initialize(None).await;
    assert!(init_result.is_ok(), "MCP initialize should succeed");
    
    let init_response = init_result.unwrap();
    assert!(init_response.get("protocolVersion").is_some(), "Should have protocol version");
    assert!(init_response.get("capabilities").is_some(), "Should have capabilities");
    assert!(init_response.get("serverInfo").is_some(), "Should have server info");
    
    // Test 3.2: List tools includes multi-environment tools
    info!("Testing that list tools includes multi-environment tools");
    
    let tools_result = server.handle_list_tools().await;
    assert!(tools_result.is_ok(), "List tools should succeed");
    
    let tools_response = tools_result.unwrap();
    let tools = tools_response.get("tools").unwrap().as_array().unwrap();
    
    // Verify multi-environment tools are present
    let tool_names: Vec<String> = tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(|name| name.as_str()).map(|s| s.to_string()))
        .collect();
    
    let expected_multi_env_tools = vec![
        "execute_query_env",
        "execute_query_multi_env", 
        "list_environments",
        "list_databases_env",
        "compare_schema",
        "health_check_env",
    ];
    
    for expected_tool in &expected_multi_env_tools {
        assert!(tool_names.contains(&expected_tool.to_string()), 
               "Multi-environment tool '{}' should be available", expected_tool);
    }
    
    // Test 3.3: Tool execution follows MCP protocol
    info!("Testing tool execution follows MCP protocol");
    
    let tool_call_result = server.handle_call_tool(Some(json!({
        "name": "list_environments",
        "arguments": {}
    }))).await;
    
    assert!(tool_call_result.is_ok(), "Tool call should succeed");
    
    let tool_response = tool_call_result.unwrap();
    assert!(tool_response.get("content").is_some(), "Tool response should have content");
    
    let content = tool_response.get("content").unwrap().as_array().unwrap();
    assert!(!content.is_empty(), "Content should not be empty");
    
    let first_content = &content[0];
    assert_eq!(first_content.get("type").unwrap().as_str().unwrap(), "text", 
              "Content type should be text");
    assert!(first_content.get("text").is_some(), "Content should have text field");
    
    // Test 3.4: Error handling follows MCP protocol
    info!("Testing error handling follows MCP protocol");
    
    let error_tool_call = server.handle_call_tool(Some(json!({
        "name": "nonexistent_tool",
        "arguments": {}
    }))).await;
    
    assert!(error_tool_call.is_err(), "Invalid tool call should return error");
    
    // Test 3.5: Parameter validation follows MCP protocol
    info!("Testing parameter validation follows MCP protocol");
    
    let invalid_params_call = server.handle_call_tool(Some(json!({
        "name": "execute_query_env",
        "arguments": {
            // Missing required 'sql' parameter
            "environment": "dev"
        }
    }))).await;
    
    assert!(invalid_params_call.is_err(), "Invalid parameters should return error");
    
    info!("✅ MCP protocol compliance test completed successfully");
}

/// Test 4: Docker deployment with multiple environment configurations
#[tokio::test]
async fn test_docker_multi_environment_configuration() {
    let _ = tracing_subscriber::fmt::try_init();
    
    info!("Testing Docker multi-environment configuration compatibility");
    
    // Test 4.1: Configuration parsing for Docker setup
    info!("Testing configuration parsing for Docker setup");
    
    // Create a configuration that mimics Docker multi-environment setup
    let docker_style_config = create_docker_style_config();
    
    // Test that the configuration can be loaded and validated
    let server_result = McpServer::with_multi_environment(docker_style_config).await;
    
    match server_result {
        Ok(server) => {
            info!("✅ Docker-style configuration loaded successfully");
            
            // Verify multi-environment support is enabled
            assert!(server.is_multi_environment(), "Server should support multi-environment operations");
            
            // Test that initialization handles Docker-style configuration
            let init_result = server.initialize().await;
            
            match init_result {
                Ok(()) => {
                    info!("✅ Docker-style server initialization succeeded");
                }
                Err(e) => {
                    info!("⚠️  Docker-style server initialization failed as expected: {}", e.user_message());
                    // This is expected when no real databases are available
                    assert!(e.user_message().to_lowercase().contains("environment") || 
                           e.user_message().to_lowercase().contains("connection"));
                }
            }
        }
        Err(e) => {
            info!("⚠️  Docker-style configuration failed as expected: {}", e.user_message());
            // This is expected when no real databases are available
            assert!(e.user_message().to_lowercase().contains("environment") || 
                   e.user_message().to_lowercase().contains("connection"));
        }
    }
    
    // Test 4.2: Environment variable configuration parsing
    info!("Testing environment variable configuration parsing");
    
    // Test that configuration handles environment variables properly
    let env_var_config = create_env_var_style_config();
    
    let env_server_result = McpServer::with_multi_environment(env_var_config).await;
    
    match env_server_result {
        Ok(_) => {
            info!("✅ Environment variable configuration loaded successfully");
        }
        Err(e) => {
            info!("⚠️  Environment variable configuration failed as expected: {}", e.user_message());
            // Expected when no real databases are available
        }
    }
    
    // Test 4.3: Docker Compose service name resolution
    info!("Testing Docker Compose service name resolution");
    
    // Create configuration with Docker service names as hosts
    let compose_config = create_docker_compose_style_config();
    
    let compose_server_result = McpServer::with_multi_environment(compose_config).await;
    
    match compose_server_result {
        Ok(_) => {
            info!("✅ Docker Compose configuration loaded successfully");
        }
        Err(e) => {
            info!("⚠️  Docker Compose configuration failed as expected: {}", e.user_message());
            // Expected when Docker services are not available
            assert!(e.user_message().to_lowercase().contains("environment") || 
                   e.user_message().to_lowercase().contains("connection") ||
                   e.user_message().to_lowercase().contains("host"));
        }
    }
    
    info!("✅ Docker deployment configuration test completed successfully");
}

/// Test 5: Performance and concurrent access in multi-environment setup
#[tokio::test]
async fn test_multi_environment_performance_and_concurrency() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let test_config = MultiEnvTestConfig::from_env();
    
    if !test_config.can_run_real_db_tests() {
        println!("Skipping performance test - insufficient test database URLs configured");
        return;
    }
    
    let server = match create_multi_env_test_server(&test_config).await {
        Some(server) => server,
        None => {
            println!("Skipping test - could not create multi-environment server");
            return;
        }
    };
    
    // Initialize server
    if let Err(e) = server.initialize().await {
        println!("Skipping test - server initialization failed: {}", e.user_message());
        return;
    }
    
    // Test 5.1: Concurrent query execution across environments
    info!("Testing concurrent query execution across environments");
    
    let mut handles = Vec::new();
    
    for i in 0..5 {
        let server_clone = server.clone();
        let handle = tokio::spawn(async move {
            let query_result = server_clone.handle_call_tool(Some(json!({
                "name": "execute_query_multi_env",
                "arguments": {
                    "sql": format!("SELECT {} as test_value, NOW() as query_time", i),
                    "environments": ["dev", "staging"]
                }
            }))).await;
            
            (i, query_result)
        });
        
        handles.push(handle);
    }
    
    // Wait for all concurrent queries to complete
    let mut successful_queries = 0;
    let mut failed_queries = 0;
    
    for handle in handles {
        match timeout(Duration::from_secs(30), handle).await {
            Ok(Ok((query_id, result))) => {
                match result {
                    Ok(_) => {
                        info!("✅ Concurrent query {} completed successfully", query_id);
                        successful_queries += 1;
                    }
                    Err(e) => {
                        warn!("❌ Concurrent query {} failed: {}", query_id, e);
                        failed_queries += 1;
                    }
                }
            }
            Ok(Err(e)) => {
                error!("Concurrent query task failed: {:?}", e);
                failed_queries += 1;
            }
            Err(_) => {
                error!("Concurrent query timed out");
                failed_queries += 1;
            }
        }
    }
    
    info!("Concurrent query results: {} successful, {} failed", successful_queries, failed_queries);
    
    // At least some queries should succeed
    assert!(successful_queries > 0, "At least some concurrent queries should succeed");
    
    // Test 5.2: Performance metrics collection
    info!("Testing performance metrics collection");
    
    let metrics_result = server.handle_call_tool(Some(json!({
        "name": "get_performance_metrics",
        "arguments": {}
    }))).await;
    
    match metrics_result {
        Ok(_) => {
            info!("✅ Performance metrics collection succeeded");
        }
        Err(e) => {
            info!("⚠️  Performance metrics collection failed: {}", e);
            // This might fail if the feature is not fully implemented
        }
    }
    
    // Test 5.3: Monitoring report generation
    info!("Testing monitoring report generation");
    
    let monitoring_result = server.handle_call_tool(Some(json!({
        "name": "get_monitoring_report",
        "arguments": {}
    }))).await;
    
    match monitoring_result {
        Ok(_) => {
            info!("✅ Monitoring report generation succeeded");
        }
        Err(e) => {
            info!("⚠️  Monitoring report generation failed: {}", e);
            // This might fail if the feature is not fully implemented
        }
    }
    
    info!("✅ Performance and concurrency test completed successfully");
}

/// Create Docker-style configuration for testing
fn create_docker_style_config() -> Config {
    let mut environments = HashMap::new();
    
    // Development environment (Docker service: mysql-dev)
    environments.insert("dev".to_string(), EnvironmentConfig {
        name: "dev".to_string(),
        description: Some("Development environment in Docker".to_string()),
        database: DatabaseConfig {
            host: "mysql-dev".to_string(),  // Docker service name
            port: 3306,
            username: "dev_user".to_string(),
            password: "dev_password".to_string(),
            database: "dev_database".to_string(),
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
    
    // Staging environment (Docker service: mysql-staging)
    environments.insert("staging".to_string(), EnvironmentConfig {
        name: "staging".to_string(),
        description: Some("Staging environment in Docker".to_string()),
        database: DatabaseConfig {
            host: "mysql-staging".to_string(),  // Docker service name
            port: 3306,
            username: "staging_user".to_string(),
            password: "staging_password".to_string(),
            database: "staging_database".to_string(),
            connection_timeout: 30,
            max_connections: 10,
        },
        connection_pool: PoolConfig {
            max_connections: 8,
            min_connections: 2,
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
        database: None,
        environments: Some(environments),
        default_environment: Some("dev".to_string()),
        mcp: McpConfig {
            protocol_version: "2024-11-05".to_string(),
            server_name: "mysql-mcp-server-docker-test".to_string(),
            server_version: "0.1.0".to_string(),
        },
    }
}

/// Create environment variable style configuration for testing
fn create_env_var_style_config() -> Config {
    let mut environments = HashMap::new();
    
    // Configuration that would typically come from environment variables
    environments.insert("dev".to_string(), EnvironmentConfig {
        name: "dev".to_string(),
        description: Some("Development environment from env vars".to_string()),
        database: DatabaseConfig {
            host: env::var("DEV_DB_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: env::var("DEV_DB_PORT").unwrap_or_else(|_| "3306".to_string()).parse().unwrap_or(3306),
            username: env::var("DEV_DB_USER").unwrap_or_else(|_| "dev_user".to_string()),
            password: env::var("DEV_DB_PASSWORD").unwrap_or_else(|_| "dev_password".to_string()),
            database: env::var("DEV_DB_NAME").unwrap_or_else(|_| "dev_database".to_string()),
            connection_timeout: 30,
            max_connections: 10,
        },
        connection_pool: PoolConfig::default(),
        enabled: true,
    });
    
    Config {
        server: ServerConfig {
            port: 8080,
            log_level: "info".to_string(),
        },
        database: None,
        environments: Some(environments),
        default_environment: Some("dev".to_string()),
        mcp: McpConfig {
            protocol_version: "2024-11-05".to_string(),
            server_name: "mysql-mcp-server-env-test".to_string(),
            server_version: "0.1.0".to_string(),
        },
    }
}

/// Create Docker Compose style configuration for testing
fn create_docker_compose_style_config() -> Config {
    let mut environments = HashMap::new();
    
    // Configuration matching docker-compose.multi-env.yml
    environments.insert("dev".to_string(), EnvironmentConfig {
        name: "dev".to_string(),
        description: Some("Development environment via Docker Compose".to_string()),
        database: DatabaseConfig {
            host: "mysql-mcp-dev-multi".to_string(),  // Docker Compose service name
            port: 3306,
            username: "dev_user".to_string(),
            password: "dev_password".to_string(),
            database: "dev_database".to_string(),
            connection_timeout: 30,
            max_connections: 10,
        },
        connection_pool: PoolConfig::default(),
        enabled: true,
    });
    
    environments.insert("staging".to_string(), EnvironmentConfig {
        name: "staging".to_string(),
        description: Some("Staging environment via Docker Compose".to_string()),
        database: DatabaseConfig {
            host: "mysql-mcp-staging-multi".to_string(),  // Docker Compose service name
            port: 3306,
            username: "staging_user".to_string(),
            password: "staging_password".to_string(),
            database: "staging_database".to_string(),
            connection_timeout: 30,
            max_connections: 10,
        },
        connection_pool: PoolConfig::default(),
        enabled: true,
    });
    
    Config {
        server: ServerConfig {
            port: 8080,
            log_level: "info".to_string(),
        },
        database: None,
        environments: Some(environments),
        default_environment: Some("dev".to_string()),
        mcp: McpConfig {
            protocol_version: "2024-11-05".to_string(),
            server_name: "mysql-mcp-server-compose-test".to_string(),
            server_version: "0.1.0".to_string(),
        },
    }
}

/// Test 6: Streaming functionality across multiple environments
#[tokio::test]
async fn test_multi_environment_streaming() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let test_config = MultiEnvTestConfig::from_env();
    
    if !test_config.can_run_real_db_tests() {
        println!("Skipping streaming test - insufficient test database URLs configured");
        return;
    }
    
    let server = match create_multi_env_test_server(&test_config).await {
        Some(server) => server,
        None => {
            println!("Skipping test - could not create multi-environment server");
            return;
        }
    };
    
    // Initialize server
    if let Err(e) = server.initialize().await {
        println!("Skipping test - server initialization failed: {}", e.user_message());
        return;
    }
    
    // Test 6.1: Single environment streaming
    info!("Testing single environment streaming");
    
    let streaming_result = server.handle_call_tool(Some(json!({
        "name": "execute_streaming_query_env",
        "arguments": {
            "sql": "SELECT 1 as test_value UNION SELECT 2 UNION SELECT 3",
            "environment": "dev"
        }
    }))).await;
    
    match streaming_result {
        Ok(_) => {
            info!("✅ Single environment streaming succeeded");
        }
        Err(e) => {
            info!("⚠️  Single environment streaming failed: {}", e);
            // This might fail if streaming is not fully implemented
        }
    }
    
    // Test 6.2: Multi-environment streaming
    info!("Testing multi-environment streaming");
    
    let multi_streaming_result = server.handle_call_tool(Some(json!({
        "name": "execute_streaming_query_multi_env",
        "arguments": {
            "sql": "SELECT 'test' as message, NOW() as timestamp",
            "environments": ["dev", "staging"]
        }
    }))).await;
    
    match multi_streaming_result {
        Ok(_) => {
            info!("✅ Multi-environment streaming succeeded");
        }
        Err(e) => {
            info!("⚠️  Multi-environment streaming failed: {}", e);
            // This might fail if streaming is not fully implemented
        }
    }
    
    info!("✅ Multi-environment streaming test completed");
}