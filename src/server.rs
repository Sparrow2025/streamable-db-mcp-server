//! MCP server implementation

use crate::{ConnectionConfig, Result, ServerError};
use crate::connection::ConnectionManager;
use crate::query::{QueryProcessor, QueryRequest};
use crate::streaming::{ResultStreamer, StreamingConfig};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, debug};

/// JSON-RPC request structure
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// JSON-RPC response structure
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error structure
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// MCP server for MySQL database operations
pub struct McpServer {
    config: ConnectionConfig,
    connection_manager: Arc<Mutex<ConnectionManager>>,
    result_streamer: ResultStreamer,
}

impl McpServer {
    /// Create a new MCP server instance
    pub fn new(config: ConnectionConfig) -> Self {
        let connection_manager = Arc::new(Mutex::new(ConnectionManager::new(config.clone())));
        let result_streamer = ResultStreamer::new();
        
        Self { 
            config,
            connection_manager,
            result_streamer,
        }
    }

    /// Create a new MCP server instance with custom streaming configuration
    pub fn with_streaming_config(config: ConnectionConfig, streaming_config: StreamingConfig) -> Self {
        let connection_manager = Arc::new(Mutex::new(ConnectionManager::new(config.clone())));
        let result_streamer = ResultStreamer::with_config(streaming_config);
        
        Self { 
            config,
            connection_manager,
            result_streamer,
        }
    }

    /// Get the server configuration
    pub fn config(&self) -> &ConnectionConfig {
        &self.config
    }

    /// Initialize the database connection
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing MySQL MCP server");
        
        let mut manager = self.connection_manager.lock().await;
        manager.connect().await?;
        manager.test_connection().await?;
        
        info!("MySQL MCP server initialized successfully");
        Ok(())
    }

    /// Start the MCP server with HTTP transport
    pub async fn start(&self, port: u16) -> Result<()> {
        info!("Starting MCP server on port {}", port);

        // Initialize the connection first
        self.initialize().await?;

        // Create HTTP server using warp
        use warp::Filter;
        
        let server = self.clone();
        
        let server_for_mcp = server.clone();
        let mcp_route = warp::path("mcp")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |request: JsonRpcRequest| {
                let server = server_for_mcp.clone();
                async move {
                    // Check if this is a notification (no ID)
                    let is_notification = request.method.starts_with("notifications/") && request.id.is_none();
                    
                    match server.handle_jsonrpc_request(request).await {
                        Ok(response) => {
                            if is_notification {
                                // For notifications, return empty response with 204 No Content
                                Ok(warp::reply::with_status(
                                    warp::reply::json(&json!({})),
                                    warp::http::StatusCode::NO_CONTENT
                                ))
                            } else {
                                Ok(warp::reply::with_status(
                                    warp::reply::json(&response),
                                    warp::http::StatusCode::OK
                                ))
                            }
                        },
                        Err(e) => {
                            error!("Error handling request: {}", e);
                            Err(warp::reject::custom(ServerError::internal_error(
                                "Request handling failed".to_string(),
                                Some(e.to_string())
                            )))
                        }
                    }
                }
            });

        // Add GET handler for MCP endpoint (some clients might expect this)
        let server_for_get = server.clone();
        let mcp_get_route = warp::path("mcp")
            .and(warp::get())
            .and_then(move || {
                let server = server_for_get.clone();
                async move {
                    // Return server info for GET requests
                    let response = json!({
                        "jsonrpc": "2.0",
                        "result": {
                            "protocolVersion": "2024-11-05",
                            "capabilities": {
                                "tools": {}
                            },
                            "serverInfo": {
                                "name": "mysql-mcp-server",
                                "version": "0.1.0"
                            }
                        }
                    });
                    Ok::<_, warp::Rejection>(warp::reply::json(&response))
                }
            });

        // Add OPTIONS handler for CORS preflight
        let mcp_options = warp::path("mcp")
            .and(warp::options())
            .map(|| {
                warp::reply::with_status("", warp::http::StatusCode::OK)
            });

        let server_for_stream = server.clone();
        let stream_route = warp::path("stream")
            .and(warp::path("query"))
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |request: QueryRequest| {
                let server = server_for_stream.clone();
                async move {
                    match server.handle_streaming_query(request).await {
                        Ok(chunks) => {
                            // Convert chunks to Server-Sent Events
                            use futures::stream;

                            
                            let event_stream = stream::iter(chunks.into_iter().map(|chunk| {
                                let json_str = serde_json::to_string(&chunk).unwrap_or_default();
                                Ok::<warp::sse::Event, warp::Error>(warp::sse::Event::default()
                                    .event("chunk")
                                    .data(json_str))
                            }));
                            
                            Ok(warp::sse::reply(warp::sse::keep_alive().stream(event_stream)))
                        }
                        Err(e) => {
                            error!("Error handling streaming query: {}", e);
                            Err(warp::reject::custom(ServerError::internal_error(
                                "Streaming query handling failed".to_string(),
                                Some(e.to_string())
                            )))
                        }
                    }
                }
            });

        let cors = warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type", "authorization", "x-requested-with", "accept"])
            .allow_methods(vec!["POST", "GET", "OPTIONS"])
            .expose_headers(vec!["content-type"]);

        let routes = mcp_route.or(mcp_get_route).or(mcp_options).or(stream_route).with(cors);

        info!("MCP server listening on http://0.0.0.0:{}/mcp", port);
        info!("Streaming endpoint available at http://0.0.0.0:{}/stream/query", port);
        
        // Start the server
        warp::serve(routes)
            .run(([0, 0, 0, 0], port))
            .await;

        info!("Server stopped");
        Ok(())
    }

    /// Shutdown the server gracefully
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down MCP server");
        
        // Close database connections
        let mut manager = self.connection_manager.lock().await;
        if let Err(e) = manager.disconnect().await {
            error!("Error closing database connection during shutdown: {}", e);
            // Continue with shutdown even if connection close fails
        }
        
        info!("MCP server shutdown complete");
        Ok(())
    }

    /// Handle JSON-RPC requests
    async fn handle_jsonrpc_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        info!("Handling JSON-RPC request: method={}, id={:?}, params={:?}", request.method, request.id, request.params);

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params).await,
            "notifications/initialized" => self.handle_initialized_notification(request.params).await,
            "tools/list" => self.handle_list_tools().await,
            "tools/call" => self.handle_call_tool(request.params).await,
            _ => {
                Err(ServerError::protocol_error(
                    format!("Unknown method: {}", request.method),
                    request.id.as_ref().and_then(|v| v.as_str()).map(|s| s.to_string())
                ))
            }
        };

        // Handle notifications (requests without ID) differently
        if request.method.starts_with("notifications/") && request.id.is_none() {
            // For notifications, we don't send a response
            match result {
                Ok(_) => {
                    info!("Notification handled successfully: {}", request.method);
                    // Return a dummy response that won't be sent
                    Ok(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: None,
                        error: None,
                    })
                },
                Err(e) => {
                    error!("Notification handling failed: {}", e.detailed_message());
                    // Return a dummy error response that won't be sent
                    Ok(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: None,
                        error: None,
                    })
                }
            }
        } else {
            // Handle regular requests
            match result {
                Ok(result_value) => Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(result_value),
                    error: None,
                }),
                Err(e) => {
                    // Log detailed error information
                    error!("JSON-RPC request failed: {}", e.detailed_message());
                    
                    // Determine appropriate error code based on error type
                    let error_code = match &e {
                        ServerError::Protocol { .. } => -32601, // Method not found
                        ServerError::Validation { .. } => -32602, // Invalid params
                        ServerError::Internal { .. } => -32603, // Internal error
                        ServerError::Timeout { .. } => -32000, // Server error (timeout)
                        ServerError::ResourceExhaustion { .. } => -32000, // Server error (resource)
                        _ => -32603, // Internal error (default)
                    };
                    
                    Ok(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: error_code,
                            message: e.user_message(),
                            data: None,
                        }),
                    })
                }
            }
        }
    }

    /// Handle initialize request
    pub async fn handle_initialize(&self, _params: Option<Value>) -> Result<Value> {
        info!("Handling initialize request");
        
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "mysql-mcp-server",
                "version": "0.1.0"
            }
        }))
    }

    /// Handle initialized notification
    pub async fn handle_initialized_notification(&self, _params: Option<Value>) -> Result<Value> {
        info!("Handling initialized notification");
        
        // For notifications, we typically don't return a result
        // But since our JSON-RPC handler expects a result, we'll return an empty object
        Ok(json!({}))
    }

    /// Handle list tools request
    pub async fn handle_list_tools(&self) -> Result<Value> {
        debug!("Handling list tools request");
        
        Ok(json!({
            "tools": [
                {
                    "name": "execute_query",
                    "description": "Execute read-only SQL queries (SELECT, SHOW, DESCRIBE, EXPLAIN) against the MySQL database. Write operations (INSERT, UPDATE, DELETE) are not allowed for security reasons.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "sql": {
                                "type": "string",
                                "description": "Read-only SQL query to execute (SELECT, SHOW, DESCRIBE, EXPLAIN only)"
                            },
                            "parameters": {
                                "type": "array",
                                "description": "Optional query parameters",
                                "items": {}
                            },
                            "stream_results": {
                                "type": "boolean",
                                "description": "Whether to stream results for large datasets",
                                "default": false
                            }
                        },
                        "required": ["sql"]
                    }
                },
                {
                    "name": "test_connection",
                    "description": "Test the database connection",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "list_databases",
                    "description": "List all available databases",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "list_tables",
                    "description": "List all tables in a specific database",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "database": {
                                "type": "string",
                                "description": "Database name (optional, uses current database if not specified)"
                            }
                        }
                    }
                },
                {
                    "name": "describe_table",
                    "description": "Get detailed information about a table structure",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "table": {
                                "type": "string",
                                "description": "Table name"
                            },
                            "database": {
                                "type": "string",
                                "description": "Database name (optional, uses current database if not specified)"
                            }
                        },
                        "required": ["table"]
                    }
                },
                {
                    "name": "list_columns",
                    "description": "List all columns in a specific table",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "table": {
                                "type": "string",
                                "description": "Table name"
                            },
                            "database": {
                                "type": "string",
                                "description": "Database name (optional, uses current database if not specified)"
                            }
                        },
                        "required": ["table"]
                    }
                }
            ]
        }))
    }

    /// Handle call tool request
    pub async fn handle_call_tool(&self, params: Option<Value>) -> Result<Value> {
        let params = params.ok_or_else(|| {
            ServerError::validation_error(
                "Missing parameters for tool call".to_string(),
                None
            )
        })?;
        
        let tool_name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ServerError::validation_error(
                    "Missing tool name".to_string(),
                    Some("name field not found or not a string".to_string())
                )
            })?;
        
        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
        
        debug!("Calling tool: {} with arguments: {}", tool_name, arguments);
        
        let result = match tool_name {
            "execute_query" => self.handle_execute_query(arguments).await?,
            "test_connection" => self.handle_test_connection(arguments).await?,
            "list_databases" => self.handle_list_databases(arguments).await?,
            "list_tables" => self.handle_list_tables(arguments).await?,
            "describe_table" => self.handle_describe_table(arguments).await?,
            "list_columns" => self.handle_list_columns(arguments).await?,
            _ => {
                return Err(ServerError::validation_error(
                    format!("Unknown tool: {}", tool_name),
                    Some(tool_name.to_string())
                ));
            }
        };

        // Serialize the result with proper error handling
        let result_text = serde_json::to_string_pretty(&result)
            .map_err(|e| ServerError::serialization_error(e, "tool call result".to_string()))?;

        Ok(json!({
            "content": [
                {
                    "type": "text",
                    "text": result_text
                }
            ]
        }))
    }

    /// Handle query execution tool
    pub async fn handle_execute_query(&self, arguments: Value) -> Result<Value> {
        debug!("Handling execute_query tool call with arguments: {}", arguments);

        // Extract SQL query from arguments
        let sql = arguments.get("sql")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ServerError::validation_error(
                    "Missing required parameter: sql".to_string(),
                    Some("sql field not found or not a string".to_string())
                )
            })?
            .to_string();

        // Extract optional parameters
        let parameters = arguments.get("parameters")
            .and_then(|v| v.as_array())
            .map(|arr| arr.clone());

        // Extract optional stream_results flag
        let stream_results = arguments.get("stream_results")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Create QueryRequest with extracted values
        let query_request = QueryRequest {
            sql,
            parameters,
            stream_results,
        };

        // Validate that only SELECT queries are allowed
        let sql_trimmed = query_request.sql.trim().to_uppercase();
        if !Self::is_read_only_query(&sql_trimmed) {
            return Err(ServerError::validation_error(
                "Only SELECT queries are allowed for security reasons".to_string(),
                Some(format!("Rejected query type. Only SELECT statements are permitted. Query: {}", 
                    query_request.sql.chars().take(100).collect::<String>()))
            ));
        }

        // Get a connection and execute the query
        let mut manager = self.connection_manager.lock().await;
        let connection = manager.connection_mut()?;
        
        // Check if streaming is requested and the query is a SELECT
        let is_select = sql_trimmed.starts_with("SELECT");
        
        if query_request.stream_results && is_select {
            // Use streaming execution
            info!("Executing query with streaming enabled");
            
            use std::time::Instant;
            
            let start_time = Instant::now();
            let chunks = self.result_streamer.execute_streaming_query(connection, &query_request.sql).await?;
            
            // For the MCP response, we'll return the chunks as a single result
            // In a real streaming scenario, these would be sent incrementally
            let json_result = serde_json::to_value(json!({
                "streaming": true,
                "chunks": chunks,
                "execution_time_ms": start_time.elapsed().as_millis() as u64
            }))
            .map_err(|e| {
                ServerError::serialization_error(e, "streaming query result".to_string())
            })?;

            Ok(json_result)
        } else {
            // Use regular execution
            let result = QueryProcessor::execute_query(connection, &query_request).await?;
            
            // Convert result to JSON
            let json_result = serde_json::to_value(result)
                .map_err(|e| {
                    ServerError::serialization_error(e, "query result".to_string())
                })?;

            Ok(json_result)
        }
    }

    /// Handle connection test tool
    pub async fn handle_test_connection(&self, _arguments: Value) -> Result<Value> {
        debug!("Handling test_connection tool call");

        let mut manager = self.connection_manager.lock().await;
        manager.test_connection().await?;

        Ok(json!({
            "status": "success",
            "message": "Database connection is healthy"
        }))
    }

    /// Handle streaming query requests
    pub async fn handle_streaming_query(&self, query_request: QueryRequest) -> Result<Vec<crate::streaming::ResultChunk>> {
        debug!("Handling streaming query: {}", query_request.sql);

        // Validate that this is a SELECT query for streaming
        let sql_trimmed = query_request.sql.trim().to_uppercase();
        if !sql_trimmed.starts_with("SELECT") {
            return Err(ServerError::validation_error(
                "Streaming is only supported for SELECT queries".to_string(),
                Some(query_request.sql.clone())
            ));
        }

        // Get a connection and execute the streaming query
        let mut manager = self.connection_manager.lock().await;
        let connection = manager.connection_mut()?;
        
        let chunks = self.result_streamer.execute_streaming_query(connection, &query_request.sql).await?;
        
        Ok(chunks)
    }

    /// Handle list databases tool
    pub async fn handle_list_databases(&self, _arguments: Value) -> Result<Value> {
        debug!("Handling list_databases tool call");

        let query_request = QueryRequest {
            sql: "SELECT SCHEMA_NAME AS Database_Name FROM INFORMATION_SCHEMA.SCHEMATA ORDER BY SCHEMA_NAME".to_string(),
            parameters: None,
            stream_results: false,
        };

        let mut manager = self.connection_manager.lock().await;
        let connection = manager.connection_mut()?;
        
        let result = QueryProcessor::execute_query(connection, &query_request).await?;
        
        // Convert result to JSON
        let json_result = serde_json::to_value(result)
            .map_err(|e| {
                ServerError::serialization_error(e, "list databases result".to_string())
            })?;

        Ok(json_result)
    }

    /// Handle list tables tool
    pub async fn handle_list_tables(&self, arguments: Value) -> Result<Value> {
        debug!("Handling list_tables tool call with arguments: {}", arguments);

        // Parse database name if provided
        let database = arguments.get("database")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let sql = if let Some(db) = database {
            format!(
                "SELECT TABLE_NAME FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_SCHEMA = '{}' ORDER BY TABLE_NAME",
                db
            )
        } else {
            "SELECT TABLE_NAME FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_SCHEMA = DATABASE() ORDER BY TABLE_NAME".to_string()
        };

        let query_request = QueryRequest {
            sql,
            parameters: None,
            stream_results: false,
        };

        let mut manager = self.connection_manager.lock().await;
        let connection = manager.connection_mut()?;
        
        let result = QueryProcessor::execute_query(connection, &query_request).await?;
        
        // Convert result to JSON
        let json_result = serde_json::to_value(result)
            .map_err(|e| {
                ServerError::serialization_error(e, "list tables result".to_string())
            })?;

        Ok(json_result)
    }

    /// Handle describe table tool
    pub async fn handle_describe_table(&self, arguments: Value) -> Result<Value> {
        debug!("Handling describe_table tool call with arguments: {}", arguments);

        // Parse table name (required)
        let table = arguments.get("table")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ServerError::validation_error(
                    "Missing required parameter: table".to_string(),
                    None
                )
            })?;

        // Parse database name if provided
        let database = arguments.get("database")
            .and_then(|v| v.as_str());

        let sql = if let Some(db) = database {
            format!(
                "SELECT COLUMN_NAME as Field, DATA_TYPE as Type, IS_NULLABLE as `Null`, 
                        COLUMN_KEY as `Key`, COLUMN_DEFAULT as `Default`, EXTRA as Extra
                 FROM INFORMATION_SCHEMA.COLUMNS 
                 WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' 
                 ORDER BY ORDINAL_POSITION",
                db, table
            )
        } else {
            format!(
                "SELECT COLUMN_NAME as Field, DATA_TYPE as Type, IS_NULLABLE as `Null`, 
                        COLUMN_KEY as `Key`, COLUMN_DEFAULT as `Default`, EXTRA as Extra
                 FROM INFORMATION_SCHEMA.COLUMNS 
                 WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = '{}' 
                 ORDER BY ORDINAL_POSITION",
                table
            )
        };

        let query_request = QueryRequest {
            sql,
            parameters: None,
            stream_results: false,
        };

        let mut manager = self.connection_manager.lock().await;
        let connection = manager.connection_mut()?;
        
        let result = QueryProcessor::execute_query(connection, &query_request).await?;
        
        // Convert result to JSON
        let json_result = serde_json::to_value(result)
            .map_err(|e| {
                ServerError::serialization_error(e, "describe table result".to_string())
            })?;

        Ok(json_result)
    }

    /// Handle list columns tool
    pub async fn handle_list_columns(&self, arguments: Value) -> Result<Value> {
        debug!("Handling list_columns tool call with arguments: {}", arguments);

        // Parse table name (required)
        let table = arguments.get("table")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ServerError::validation_error(
                    "Missing required parameter: table".to_string(),
                    None
                )
            })?;

        // Parse database name if provided
        let database = arguments.get("database")
            .and_then(|v| v.as_str());

        let sql = if let Some(db) = database {
            format!(
                "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT, COLUMN_KEY, EXTRA 
                 FROM INFORMATION_SCHEMA.COLUMNS 
                 WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' 
                 ORDER BY ORDINAL_POSITION",
                db, table
            )
        } else {
            format!(
                "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT, COLUMN_KEY, EXTRA 
                 FROM INFORMATION_SCHEMA.COLUMNS 
                 WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = '{}' 
                 ORDER BY ORDINAL_POSITION",
                table
            )
        };

        let query_request = QueryRequest {
            sql,
            parameters: None,
            stream_results: false,
        };

        let mut manager = self.connection_manager.lock().await;
        let connection = manager.connection_mut()?;
        
        let result = QueryProcessor::execute_query(connection, &query_request).await?;
        
        // Convert result to JSON
        let json_result = serde_json::to_value(result)
            .map_err(|e| {
                ServerError::serialization_error(e, "list columns result".to_string())
            })?;

        Ok(json_result)
    }
}

impl McpServer {
    /// Check if a SQL query is read-only (only SELECT statements and related read operations)
    fn is_read_only_query(sql: &str) -> bool {
        let sql_trimmed = sql.trim().to_uppercase();
        
        // Allow SELECT statements
        if sql_trimmed.starts_with("SELECT") {
            return true;
        }
        
        // Allow SHOW statements (for database introspection)
        if sql_trimmed.starts_with("SHOW") {
            return true;
        }
        
        // Allow DESCRIBE/DESC statements
        if sql_trimmed.starts_with("DESCRIBE") || sql_trimmed.starts_with("DESC") {
            return true;
        }
        
        // Allow EXPLAIN statements
        if sql_trimmed.starts_with("EXPLAIN") {
            return true;
        }
        
        // Reject all other statements (INSERT, UPDATE, DELETE, DROP, CREATE, ALTER, etc.)
        false
    }
}

impl Clone for McpServer {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            connection_manager: Arc::clone(&self.connection_manager),
            result_streamer: ResultStreamer::new(), // Create new instance for clone
        }
    }
}

impl warp::reject::Reject for ServerError {}