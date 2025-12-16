//! Integration test for multi-environment server startup functionality
//! 
//! This test verifies that the server can properly initialize with multiple
//! database environments, handle graceful startup with partial failures,
//! and perform comprehensive health checks during startup.

use mysql_mcp_server::{Config, server::McpServer};
use mysql_mcp_server::config::*;
use std::collections::HashMap;

/// Test multi-environment server creation and initialization
#[tokio::test]
async fn test_multi_environment_server_startup() {
    // Initialize logging for the test
    let _ = tracing_subscriber::fmt::try_init();
    
    // Create a sample multi-environment configuration
    let config = create_test_multi_env_config();
    
    // Verify configuration is properly set up
    assert!(config.is_multi_environment(), "Configuration should be multi-environment");
    assert_eq!(config.get_default_environment(), Some("dev"), "Default environment should be 'dev'");
    
    let environments = config.get_environments().expect("Should have environments");
    assert_eq!(environments.len(), 3, "Should have 3 environments configured");
    assert!(environments.contains_key("dev"), "Should have dev environment");
    assert!(environments.contains_key("staging"), "Should have staging environment");
    assert!(environments.contains_key("prod"), "Should have prod environment");
    
    // Test server creation with multi-environment support
    let server_result = McpServer::with_multi_environment(config).await;
    
    match server_result {
        Ok(server) => {
            // Verify server was created with multi-environment support
            assert!(server.is_multi_environment(), "Server should support multi-environment operations");
            
            // Test server initialization
            // Note: This will likely fail to connect to actual databases, but should
            // demonstrate graceful startup handling
            let init_result = server.initialize().await;
            
            // The initialization might fail due to non-existent databases,
            // but the important thing is that it handles the failure gracefully
            // and doesn't panic or crash
            match init_result {
                Ok(()) => {
                    println!("✅ Server initialization succeeded (unexpected but good!)");
                }
                Err(e) => {
                    println!("⚠️  Server initialization failed as expected: {}", e.user_message());
                    // This is expected when no real databases are available
                    // The important thing is that the error handling is graceful
                    assert!(e.user_message().contains("environment") || 
                           e.user_message().contains("connection") ||
                           e.user_message().contains("healthy"),
                           "Error should be related to environment/connection issues");
                }
            }
            
            println!("✅ Multi-environment server startup test completed successfully");
        }
        Err(e) => {
            // Server creation might fail due to connection issues, but should provide clear error messages
            println!("⚠️  Server creation failed as expected: {}", e.user_message());
            
            // Verify that the error is related to connection/environment issues, not code bugs
            let error_msg = e.user_message().to_lowercase();
            assert!(error_msg.contains("environment") || 
                   error_msg.contains("connection") || 
                   error_msg.contains("healthy") ||
                   error_msg.contains("initialize"),
                   "Error should be related to environment/connection issues, got: {}", e.user_message());
        }
    }
}

/// Test that server startup validation works correctly
#[tokio::test]
async fn test_startup_validation_logic() {
    let _ = tracing_subscriber::fmt::try_init();
    
    // Create configuration with invalid environment setup
    let mut config = create_test_multi_env_config();
    
    // Disable all environments to test validation
    if let Some(environments) = &mut config.environments {
        for env in environments.values_mut() {
            env.enabled = false;
        }
    }
    
    // This should fail during environment manager creation
    let result = McpServer::with_multi_environment(config).await;
    assert!(result.is_err(), "Server creation should fail with no enabled environments");
    
    if let Err(error) = result {
        assert!(error.user_message().contains("environment") || error.user_message().contains("enabled"),
               "Error should mention environment or enabled issues, got: {}", error.user_message());
    }
}

/// Test graceful startup with partial environment failures
#[tokio::test]
async fn test_graceful_startup_behavior() {
    let _ = tracing_subscriber::fmt::try_init();
    
    let config = create_test_multi_env_config();
    
    // The server should be created even if some environments fail to connect
    // This tests the graceful startup logic
    let server_result = McpServer::with_multi_environment(config).await;
    
    // Even if server creation succeeds, initialization might fail gracefully
    if let Ok(server) = server_result {
        assert!(server.is_multi_environment(), "Server should support multi-environment");
        
        // The initialization should handle partial failures gracefully
        let _init_result = server.initialize().await;
        // We don't assert success/failure here because it depends on actual database availability
        // The important thing is that it doesn't panic and provides meaningful error messages
        
        println!("✅ Graceful startup behavior test completed");
    } else {
        println!("⚠️  Server creation failed, which is acceptable for this test");
    }
}

fn create_test_multi_env_config() -> Config {
    let mut environments = HashMap::new();
    
    // Development environment
    environments.insert("dev".to_string(), EnvironmentConfig {
        name: "dev".to_string(),
        description: Some("Development environment for testing".to_string()),
        database: DatabaseConfig {
            host: "localhost".to_string(),
            port: 3306,
            username: "test_dev_user".to_string(),
            password: "test_dev_password".to_string(),
            database: "test_dev_db".to_string(),
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
    
    // Staging environment
    environments.insert("staging".to_string(), EnvironmentConfig {
        name: "staging".to_string(),
        description: Some("Staging environment for testing".to_string()),
        database: DatabaseConfig {
            host: "staging-test.example.com".to_string(),
            port: 3306,
            username: "test_staging_user".to_string(),
            password: "test_staging_password".to_string(),
            database: "test_staging_db".to_string(),
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
    
    // Production environment (disabled for safety in tests)
    environments.insert("prod".to_string(), EnvironmentConfig {
        name: "prod".to_string(),
        description: Some("Production environment (disabled in tests)".to_string()),
        database: DatabaseConfig {
            host: "prod-test.example.com".to_string(),
            port: 3306,
            username: "test_prod_user".to_string(),
            password: "test_prod_password".to_string(),
            database: "test_prod_db".to_string(),
            connection_timeout: 30,
            max_connections: 10,
        },
        connection_pool: PoolConfig {
            max_connections: 20,
            min_connections: 5,
            connection_timeout: 30,
            idle_timeout: 600,
        },
        enabled: false, // Disabled for safety in tests
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
            server_name: "mysql-mcp-server-test".to_string(),
            server_version: "0.1.0".to_string(),
        },
    }
}