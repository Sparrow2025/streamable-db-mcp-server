//! Integration tests for MySQL MCP Server
//! 
//! Tests end-to-end query execution through MCP and streaming functionality with real database

use mysql_mcp_server::{ConnectionConfig, McpServer};
use mysql_mcp_server::query::QueryRequest;
use serde_json::json;
use std::env;
use tokio::time::{timeout, Duration};

/// Test configuration for integration tests
struct TestConfig {
    database_url: String,
}

impl TestConfig {
    /// Create test configuration from environment
    fn from_env() -> Option<Self> {
        // Only run integration tests if TEST_DATABASE_URL is set
        env::var("TEST_DATABASE_URL").ok().map(|database_url| {
            Self { database_url }
        })
    }
}

/// Helper function to create a test server
async fn create_test_server() -> Option<McpServer> {
    let config = TestConfig::from_env()?;
    let connection_config = ConnectionConfig {
        database_url: config.database_url,
    };
    
    let server = McpServer::new(connection_config);
    
    // Initialize the server
    if server.initialize().await.is_ok() {
        Some(server)
    } else {
        None
    }
}

/// Helper function to setup test database with sample data
async fn setup_test_database(server: &McpServer) -> Result<(), Box<dyn std::error::Error>> {
    // Create a test table
    let create_table_request = QueryRequest {
        sql: "CREATE TABLE IF NOT EXISTS test_users (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            email VARCHAR(100) UNIQUE NOT NULL,
            age INT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )".to_string(),
        parameters: None,
        stream_results: false,
    };
    
    // Execute through the server's query handling
    let arguments = serde_json::to_value(&create_table_request)?;
    server.handle_execute_query(arguments).await.map_err(|e| e.to_string())?;
    
    // Insert test data
    let test_users = vec![
        ("Alice Johnson", "alice@example.com", 28),
        ("Bob Smith", "bob@example.com", 35),
        ("Carol Davis", "carol@example.com", 42),
        ("David Wilson", "david@example.com", 31),
        ("Eve Brown", "eve@example.com", 29),
    ];
    
    // Clear existing data first
    let clear_request = QueryRequest {
        sql: "DELETE FROM test_users".to_string(),
        parameters: None,
        stream_results: false,
    };
    let arguments = serde_json::to_value(&clear_request)?;
    server.handle_execute_query(arguments).await.map_err(|e| e.to_string())?;
    
    // Insert test data
    for (name, email, age) in test_users {
        let insert_request = QueryRequest {
            sql: "INSERT INTO test_users (name, email, age) VALUES (?, ?, ?)".to_string(),
            parameters: Some(vec![
                json!(name),
                json!(email),
                json!(age)
            ]),
            stream_results: false,
        };
        
        let arguments = serde_json::to_value(&insert_request)?;
        server.handle_execute_query(arguments).await.map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

/// Helper function to cleanup test database
async fn cleanup_test_database(server: &McpServer) -> Result<(), Box<dyn std::error::Error>> {
    let drop_table_request = QueryRequest {
        sql: "DROP TABLE IF EXISTS test_users".to_string(),
        parameters: None,
        stream_results: false,
    };
    
    let arguments = serde_json::to_value(&drop_table_request)?;
    server.handle_execute_query(arguments).await.map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tokio::test]
async fn test_end_to_end_query_execution_through_mcp() {
    // Skip test if no test database URL is provided
    let server = match create_test_server().await {
        Some(server) => server,
        None => {
            println!("Skipping integration test - TEST_DATABASE_URL not set");
            return;
        }
    };
    
    // Setup test database
    if let Err(e) = setup_test_database(&server).await {
        panic!("Failed to setup test database: {}", e);
    }
    
    // Test 1: Execute SELECT query through MCP
    let select_request = QueryRequest {
        sql: "SELECT id, name, email, age FROM test_users ORDER BY name".to_string(),
        parameters: None,
        stream_results: false,
    };
    
    let arguments = serde_json::to_value(&select_request).expect("Failed to serialize request");
    let result = server.handle_execute_query(arguments).await;
    
    assert!(result.is_ok(), "SELECT query should succeed: {:?}", result.err());
    
    let result_value = result.unwrap();
    
    // Verify result structure
    assert!(result_value.get("columns").is_some(), "Result should have columns");
    assert!(result_value.get("rows").is_some(), "Result should have rows");
    assert!(result_value.get("execution_time_ms").is_some(), "Result should have execution time");
    
    // Verify we got the expected number of rows
    let rows = result_value.get("rows").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 5, "Should have 5 test users");
    
    // Verify column structure
    let columns = result_value.get("columns").unwrap().as_array().unwrap();
    assert_eq!(columns.len(), 4, "Should have 4 columns");
    
    // Test 2: Execute INSERT query through MCP
    let insert_request = QueryRequest {
        sql: "INSERT INTO test_users (name, email, age) VALUES (?, ?, ?)".to_string(),
        parameters: Some(vec![
            json!("Frank Miller"),
            json!("frank@example.com"),
            json!(45)
        ]),
        stream_results: false,
    };
    
    let arguments = serde_json::to_value(&insert_request).expect("Failed to serialize request");
    let result = server.handle_execute_query(arguments).await;
    
    assert!(result.is_ok(), "INSERT query should succeed: {:?}", result.err());
    
    let result_value = result.unwrap();
    assert!(result_value.get("affected_rows").is_some(), "INSERT result should have affected_rows");
    
    let affected_rows = result_value.get("affected_rows").unwrap().as_u64().unwrap();
    assert_eq!(affected_rows, 1, "INSERT should affect 1 row");
    
    // Test 3: Execute UPDATE query through MCP
    let update_request = QueryRequest {
        sql: "UPDATE test_users SET age = ? WHERE name = ?".to_string(),
        parameters: Some(vec![
            json!(46),
            json!("Frank Miller")
        ]),
        stream_results: false,
    };
    
    let arguments = serde_json::to_value(&update_request).expect("Failed to serialize request");
    let result = server.handle_execute_query(arguments).await;
    
    assert!(result.is_ok(), "UPDATE query should succeed: {:?}", result.err());
    
    let result_value = result.unwrap();
    let affected_rows = result_value.get("affected_rows").unwrap().as_u64().unwrap();
    assert_eq!(affected_rows, 1, "UPDATE should affect 1 row");
    
    // Test 4: Execute DELETE query through MCP
    let delete_request = QueryRequest {
        sql: "DELETE FROM test_users WHERE name = ?".to_string(),
        parameters: Some(vec![json!("Frank Miller")]),
        stream_results: false,
    };
    
    let arguments = serde_json::to_value(&delete_request).expect("Failed to serialize request");
    let result = server.handle_execute_query(arguments).await;
    
    assert!(result.is_ok(), "DELETE query should succeed: {:?}", result.err());
    
    let result_value = result.unwrap();
    let affected_rows = result_value.get("affected_rows").unwrap().as_u64().unwrap();
    assert_eq!(affected_rows, 1, "DELETE should affect 1 row");
    
    // Cleanup
    if let Err(e) = cleanup_test_database(&server).await {
        eprintln!("Warning: Failed to cleanup test database: {}", e);
    }
}

#[tokio::test]
async fn test_streaming_functionality_with_real_database() {
    // Skip test if no test database URL is provided
    let server = match create_test_server().await {
        Some(server) => server,
        None => {
            println!("Skipping streaming integration test - TEST_DATABASE_URL not set");
            return;
        }
    };
    
    // Setup test database with more data for streaming
    if let Err(e) = setup_test_database(&server).await {
        panic!("Failed to setup test database: {}", e);
    }
    
    // Add more test data to make streaming more meaningful
    for i in 6..=20 {
        let insert_request = QueryRequest {
            sql: "INSERT INTO test_users (name, email, age) VALUES (?, ?, ?)".to_string(),
            parameters: Some(vec![
                json!(format!("User{}", i)),
                json!(format!("user{}@example.com", i)),
                json!(20 + (i % 30))
            ]),
            stream_results: false,
        };
        
        let arguments = serde_json::to_value(&insert_request).expect("Failed to serialize request");
        let _ = server.handle_execute_query(arguments).await;
    }
    
    // Test 1: Execute streaming SELECT query
    let streaming_request = QueryRequest {
        sql: "SELECT id, name, email, age FROM test_users ORDER BY id".to_string(),
        parameters: None,
        stream_results: true,
    };
    
    let arguments = serde_json::to_value(&streaming_request).expect("Failed to serialize request");
    let result = server.handle_execute_query(arguments).await;
    
    assert!(result.is_ok(), "Streaming SELECT query should succeed: {:?}", result.err());
    
    let result_value = result.unwrap();
    
    // Verify streaming result structure
    assert!(result_value.get("streaming").is_some(), "Result should indicate streaming");
    assert!(result_value.get("chunks").is_some(), "Result should have chunks");
    assert!(result_value.get("execution_time_ms").is_some(), "Result should have execution time");
    
    let streaming_flag = result_value.get("streaming").unwrap().as_bool().unwrap();
    assert!(streaming_flag, "Streaming flag should be true");
    
    let chunks = result_value.get("chunks").unwrap().as_array().unwrap();
    assert!(!chunks.is_empty(), "Should have at least one chunk");
    
    // Verify chunk structure
    let first_chunk = &chunks[0];
    assert!(first_chunk.get("chunk_id").is_some(), "Chunk should have chunk_id");
    assert!(first_chunk.get("rows").is_some(), "Chunk should have rows");
    assert!(first_chunk.get("is_final").is_some(), "Chunk should have is_final flag");
    
    // Count total rows across all chunks
    let mut total_rows = 0;
    for chunk in chunks {
        let rows = chunk.get("rows").unwrap().as_array().unwrap();
        total_rows += rows.len();
    }
    
    assert!(total_rows >= 20, "Should have at least 20 rows total across chunks");
    
    // Verify the last chunk is marked as final
    let last_chunk = chunks.last().unwrap();
    let is_final = last_chunk.get("is_final").unwrap().as_bool().unwrap();
    assert!(is_final, "Last chunk should be marked as final");
    
    // Test 2: Test streaming with direct streaming endpoint
    let streaming_chunks = server.handle_streaming_query(streaming_request).await;
    
    assert!(streaming_chunks.is_ok(), "Direct streaming query should succeed: {:?}", streaming_chunks.err());
    
    let chunks = streaming_chunks.unwrap();
    assert!(!chunks.is_empty(), "Should have at least one chunk from direct streaming");
    
    // Verify chunk properties
    let mut total_streamed_rows = 0;
    for (i, chunk) in chunks.iter().enumerate() {
        assert_eq!(chunk.chunk_id, i as u64, "Chunk ID should be sequential");
        total_streamed_rows += chunk.rows.len();
        
        // Last chunk should be marked as final
        if i == chunks.len() - 1 {
            assert!(chunk.is_final, "Last chunk should be marked as final");
        } else {
            assert!(!chunk.is_final, "Non-final chunks should not be marked as final");
        }
    }
    
    assert!(total_streamed_rows >= 20, "Should have at least 20 rows total in streaming");
    
    // Test 3: Test streaming with non-SELECT query (should fail)
    let non_select_request = QueryRequest {
        sql: "UPDATE test_users SET age = age + 1".to_string(),
        parameters: None,
        stream_results: true,
    };
    
    let streaming_result = server.handle_streaming_query(non_select_request).await;
    assert!(streaming_result.is_err(), "Non-SELECT queries should not support streaming");
    
    // Cleanup
    if let Err(e) = cleanup_test_database(&server).await {
        eprintln!("Warning: Failed to cleanup test database: {}", e);
    }
}

#[tokio::test]
async fn test_mcp_protocol_compliance() {
    // Skip test if no test database URL is provided
    let server = match create_test_server().await {
        Some(server) => server,
        None => {
            println!("Skipping MCP protocol compliance test - TEST_DATABASE_URL not set");
            return;
        }
    };
    
    // Test 1: Initialize request
    let init_result = server.handle_initialize(None).await;
    assert!(init_result.is_ok(), "Initialize should succeed");
    
    let init_response = init_result.unwrap();
    assert!(init_response.get("protocolVersion").is_some(), "Should have protocol version");
    assert!(init_response.get("capabilities").is_some(), "Should have capabilities");
    assert!(init_response.get("serverInfo").is_some(), "Should have server info");
    
    // Test 2: List tools request
    let tools_result = server.handle_list_tools().await;
    assert!(tools_result.is_ok(), "List tools should succeed");
    
    let tools_response = tools_result.unwrap();
    let tools = tools_response.get("tools").unwrap().as_array().unwrap();
    assert!(!tools.is_empty(), "Should have at least one tool");
    
    // Verify tool structure
    for tool in tools {
        assert!(tool.get("name").is_some(), "Tool should have name");
        assert!(tool.get("description").is_some(), "Tool should have description");
        assert!(tool.get("inputSchema").is_some(), "Tool should have input schema");
    }
    
    // Test 3: Test connection tool
    let test_conn_args = json!({});
    let test_result = server.handle_test_connection(test_conn_args).await;
    assert!(test_result.is_ok(), "Test connection should succeed");
    
    let test_response = test_result.unwrap();
    assert_eq!(test_response.get("status").unwrap().as_str().unwrap(), "success");
    
    // Test 4: Error handling for invalid tool
    let invalid_tool_params = json!({
        "name": "invalid_tool",
        "arguments": {}
    });
    
    let error_result = server.handle_call_tool(Some(invalid_tool_params)).await;
    assert!(error_result.is_err(), "Invalid tool should return error");
}

#[tokio::test]
async fn test_concurrent_query_execution() {
    // Skip test if no test database URL is provided
    let server = match create_test_server().await {
        Some(server) => server,
        None => {
            println!("Skipping concurrent execution test - TEST_DATABASE_URL not set");
            return;
        }
    };
    
    // Setup test database
    if let Err(e) = setup_test_database(&server).await {
        panic!("Failed to setup test database: {}", e);
    }
    
    // Create multiple concurrent queries
    let mut handles = Vec::new();
    
    for i in 0..5 {
        let server_clone = server.clone();
        let handle = tokio::spawn(async move {
            let select_request = QueryRequest {
                sql: format!("SELECT id, name, email FROM test_users WHERE id > {} ORDER BY id LIMIT 2", i),
                parameters: None,
                stream_results: false,
            };
            
            let arguments = serde_json::to_value(&select_request).expect("Failed to serialize request");
            
            // Add timeout to prevent hanging
            timeout(Duration::from_secs(10), server_clone.handle_execute_query(arguments)).await
        });
        
        handles.push(handle);
    }
    
    // Wait for all queries to complete
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        let query_result = result.expect("Timeout should not occur").expect("Query should succeed");
        results.push(query_result);
    }
    
    // Verify all queries succeeded
    assert_eq!(results.len(), 5, "All concurrent queries should complete");
    
    for result in results {
        assert!(result.get("columns").is_some(), "Each result should have columns");
        assert!(result.get("rows").is_some(), "Each result should have rows");
        assert!(result.get("execution_time_ms").is_some(), "Each result should have execution time");
    }
    
    // Cleanup
    if let Err(e) = cleanup_test_database(&server).await {
        eprintln!("Warning: Failed to cleanup test database: {}", e);
    }
}

#[tokio::test]
async fn test_error_handling_integration() {
    // Skip test if no test database URL is provided
    let server = match create_test_server().await {
        Some(server) => server,
        None => {
            println!("Skipping error handling integration test - TEST_DATABASE_URL not set");
            return;
        }
    };
    
    // Test 1: SQL syntax error
    let invalid_sql_request = QueryRequest {
        sql: "SELCT * FROM nonexistent_table".to_string(), // Intentional typo
        parameters: None,
        stream_results: false,
    };
    
    let arguments = serde_json::to_value(&invalid_sql_request).expect("Failed to serialize request");
    let result = server.handle_execute_query(arguments).await;
    
    assert!(result.is_err(), "Invalid SQL should return error");
    
    // Test 2: Non-existent table
    let nonexistent_table_request = QueryRequest {
        sql: "SELECT * FROM definitely_nonexistent_table_12345".to_string(),
        parameters: None,
        stream_results: false,
    };
    
    let arguments = serde_json::to_value(&nonexistent_table_request).expect("Failed to serialize request");
    let result = server.handle_execute_query(arguments).await;
    
    assert!(result.is_err(), "Query on non-existent table should return error");
    
    // Test 3: Invalid JSON in tool call
    let invalid_json_params = json!({
        "name": "execute_query",
        "arguments": {
            "sql": 123 // SQL should be string, not number
        }
    });
    
    let result = server.handle_call_tool(Some(invalid_json_params)).await;
    assert!(result.is_err(), "Invalid argument types should return error");
}