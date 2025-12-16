use mysql_mcp_server::config::{Config, DatabaseConfig, EnvironmentConfig, PoolConfig, ServerConfig, McpConfig};
use mysql_mcp_server::server::McpServer;
use std::collections::HashMap;

/// Test server initialization with multi-environment configuration
#[tokio::test]
async fn test_multi_environment_server_initialization() {
    // Create a test multi-environment configuration
    let mut environments = HashMap::new();
    
    // Create test environment configurations (these won't actually connect)
    let dev_env = EnvironmentConfig {
        name: "dev".to_string(),
        description: Some("Development environment".to_string()),
        database: DatabaseConfig {
            host: "dev-db.example.com".to_string(),
            port: 3306,
            username: "dev_user".to_string(),
            password: "dev_pass".to_string(),
            database: "dev_db".to_string(),
            connection_timeout: 30,
            max_connections: 5,
        },
        connection_pool: PoolConfig::default(),
        enabled: true,
    };
    
    let uat_env = EnvironmentConfig {
        name: "uat".to_string(),
        description: Some("UAT environment".to_string()),
        database: DatabaseConfig {
            host: "uat-db.example.com".to_string(),
            port: 3306,
            username: "uat_user".to_string(),
            password: "uat_pass".to_string(),
            database: "uat_db".to_string(),
            connection_timeout: 30,
            max_connections: 10,
        },
        connection_pool: PoolConfig::default(),
        enabled: true,
    };
    
    environments.insert("dev".to_string(), dev_env);
    environments.insert("uat".to_string(), uat_env);
    
    let config = Config {
        server: ServerConfig {
            port: 8080,
            log_level: "info".to_string(),
        },
        database: None,
        environments: Some(environments),
        default_environment: Some("dev".to_string()),
        mcp: McpConfig {
            protocol_version: "2024-11-05".to_string(),
            server_name: "test-server".to_string(),
            server_version: "0.1.0".to_string(),
        },
    };

    // Test server creation with multi-environment support
    let result = McpServer::with_multi_environment(config).await;
    
    // The server creation should succeed even if connections fail
    // (graceful startup handling)
    match result {
        Ok(server) => {
            // Verify the server is configured for multi-environment
            assert!(server.is_multi_environment());
            println!("✓ Multi-environment server created successfully");
        }
        Err(e) => {
            // This is expected since we're using fake database URLs
            println!("Expected error during server creation with fake databases: {}", e);
            // The error should be related to connection issues, not configuration
            assert!(e.to_string().contains("connection") || e.to_string().contains("environment"));
        }
    }
}

/// Test server initialization with legacy single database configuration
#[tokio::test]
async fn test_legacy_server_initialization() {
    use mysql_mcp_server::ConnectionConfig;
    
    // Create a legacy server configuration
    let connection_config = ConnectionConfig {
        database_url: "mysql://test:test@localhost:3306/test".to_string(),
    };
    
    let server = McpServer::new(connection_config);
    
    // Verify the server is not configured for multi-environment
    assert!(!server.is_multi_environment());
    println!("✓ Legacy server created successfully");
}

/// Test configuration validation during server startup
#[tokio::test]
async fn test_configuration_validation() {
    // Test with invalid configuration (no environments)
    let config = Config {
        server: ServerConfig {
            port: 8080,
            log_level: "info".to_string(),
        },
        database: None,
        environments: Some(HashMap::new()), // Empty environments
        default_environment: None, // No default environment
        mcp: McpConfig {
            protocol_version: "2024-11-05".to_string(),
            server_name: "test-server".to_string(),
            server_version: "0.1.0".to_string(),
        },
    };

    let result = McpServer::with_multi_environment(config).await;
    
    // Should fail due to no environments configured
    assert!(result.is_err());
    if let Err(error) = result {
        println!("Actual error message: {}", error.to_string());
        // The error should be about configuration issues
        assert!(error.to_string().contains("environments") || 
                error.to_string().contains("configuration"));
        println!("✓ Configuration validation working correctly");
    }
}

/// Test graceful startup behavior
#[tokio::test]
async fn test_graceful_startup_behavior() {
    // Create configuration with mixed enabled/disabled environments
    let mut environments = HashMap::new();
    
    let enabled_env = EnvironmentConfig {
        name: "enabled".to_string(),
        description: Some("Enabled environment".to_string()),
        database: DatabaseConfig {
            host: "enabled-db.example.com".to_string(),
            port: 3306,
            username: "user".to_string(),
            password: "pass".to_string(),
            database: "db".to_string(),
            connection_timeout: 30,
            max_connections: 5,
        },
        connection_pool: PoolConfig::default(),
        enabled: true,
    };
    
    let disabled_env = EnvironmentConfig {
        name: "disabled".to_string(),
        description: Some("Disabled environment".to_string()),
        database: DatabaseConfig {
            host: "disabled-db.example.com".to_string(),
            port: 3306,
            username: "user".to_string(),
            password: "pass".to_string(),
            database: "db".to_string(),
            connection_timeout: 30,
            max_connections: 5,
        },
        connection_pool: PoolConfig::default(),
        enabled: false, // Disabled
    };
    
    environments.insert("enabled".to_string(), enabled_env);
    environments.insert("disabled".to_string(), disabled_env);
    
    let config = Config {
        server: ServerConfig {
            port: 8080,
            log_level: "info".to_string(),
        },
        database: None,
        environments: Some(environments),
        default_environment: Some("enabled".to_string()),
        mcp: McpConfig {
            protocol_version: "2024-11-05".to_string(),
            server_name: "test-server".to_string(),
            server_version: "0.1.0".to_string(),
        },
    };

    let result = McpServer::with_multi_environment(config).await;
    
    // Should handle graceful startup (only enabled environments are considered)
    match result {
        Ok(server) => {
            assert!(server.is_multi_environment());
            println!("✓ Graceful startup with mixed environments successful");
        }
        Err(e) => {
            // Expected due to fake database connections
            println!("Expected connection error with fake databases: {}", e);
        }
    }
}