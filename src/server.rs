//! MCP server implementation

use crate::{ConnectionConfig, Result, ServerError};
use crate::connection::ConnectionManager;
use crate::environment::EnvironmentManager;
use crate::mcp_tools::EnhancedMcpTools;
use crate::pool::ConnectionPoolManager;
use crate::query::{QueryProcessor, QueryRequest};
use crate::router::QueryRouter;
use crate::streaming::{ResultStreamer, StreamingConfig};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, debug, warn};

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
    // Multi-environment components (optional for backward compatibility)
    environment_manager: Option<Arc<EnvironmentManager>>,
    pool_manager: Option<Arc<ConnectionPoolManager>>,
    query_router: Option<Arc<QueryRouter>>,
    enhanced_mcp_tools: Option<Arc<EnhancedMcpTools>>,
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
            environment_manager: None,
            pool_manager: None,
            query_router: None,
            enhanced_mcp_tools: None,
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
            environment_manager: None,
            pool_manager: None,
            query_router: None,
            enhanced_mcp_tools: None,
        }
    }

    /// Create a new MCP server instance with multi-environment support
    pub async fn with_multi_environment(config: crate::Config) -> Result<Self> {
        info!("Creating MCP server with multi-environment support");
        
        // Step 1: Create and validate environment manager
        info!("Step 1/5: Loading and validating environment configuration");
        let environment_manager = Arc::new(EnvironmentManager::load_from_config(&config)?);
        
        let enabled_environments = environment_manager.list_enabled_environments();
        info!("Found {} enabled environments: {}", 
              enabled_environments.len(), 
              enabled_environments.join(", "));
        
        // Step 2: Initialize connection pool manager with graceful failure handling
        info!("Step 2/5: Initializing connection pools for all environments");
        let pool_manager = match ConnectionPoolManager::initialize(environment_manager.clone()).await {
            Ok(manager) => {
                info!("Connection pool manager initialized successfully");
                Arc::new(manager)
            }
            Err(e) => {
                error!("Failed to initialize connection pool manager: {}", e.user_message());
                
                // Check if we have any healthy pools before failing completely
                // This allows graceful startup with partial environment failures
                match ConnectionPoolManager::initialize_with_partial_failure(environment_manager.clone()).await {
                    Ok(manager) => {
                        warn!("Connection pool manager initialized with partial failures - some environments may be unavailable");
                        Arc::new(manager)
                    }
                    Err(fatal_error) => {
                        error!("Fatal error: No environments could be initialized: {}", fatal_error.user_message());
                        return Err(fatal_error);
                    }
                }
            }
        };
        
        // Step 3: Perform startup health checks
        info!("Step 3/5: Performing startup health checks for all environments");
        let health_results = pool_manager.health_check(None).await?;
        let mut healthy_count = 0;
        let mut unhealthy_environments = Vec::new();
        
        for (env_name, health_status) in &health_results {
            match health_status {
                crate::pool::PoolHealthStatus::Healthy => {
                    info!("✅ Environment '{}' is healthy", env_name);
                    healthy_count += 1;
                }
                crate::pool::PoolHealthStatus::Degraded { warning, .. } => {
                    warn!("⚠️  Environment '{}' is degraded: {}", env_name, warning);
                    healthy_count += 1; // Degraded is still functional
                }
                crate::pool::PoolHealthStatus::Unhealthy { error, .. } => {
                    error!("❌ Environment '{}' is unhealthy: {}", env_name, error);
                    unhealthy_environments.push(env_name.clone());
                }
                crate::pool::PoolHealthStatus::Disabled => {
                    info!("⚫ Environment '{}' is disabled", env_name);
                }
                crate::pool::PoolHealthStatus::Initializing => {
                    warn!("🔄 Environment '{}' is still initializing", env_name);
                }
            }
        }
        
        // Validate that we have at least one healthy environment
        if healthy_count == 0 {
            return Err(ServerError::configuration_error(
                "environments".to_string(),
                format!("No healthy environments available. Unhealthy environments: {}", 
                       unhealthy_environments.join(", "))
            ));
        }
        
        info!("Startup health check complete: {}/{} environments are healthy", 
              healthy_count, health_results.len());
        
        if !unhealthy_environments.is_empty() {
            warn!("Server will start with degraded service. Unhealthy environments: {}", 
                  unhealthy_environments.join(", "));
        }
        
        // Step 4: Create query router and enhanced MCP tools
        info!("Step 4/5: Creating query router and enhanced MCP tools");
        let query_router = Arc::new(QueryRouter::new(pool_manager.clone(), environment_manager.clone()));
        let enhanced_mcp_tools = Arc::new(EnhancedMcpTools::new(
            query_router.clone(),
            environment_manager.clone(),
            pool_manager.clone(),
        ));
        
        // Step 5: Create legacy connection manager for backward compatibility
        info!("Step 5/5: Setting up legacy compatibility layer");
        let legacy_config = if let Some(default_env) = environment_manager.get_default_environment() {
            if let Some(env_config) = environment_manager.get_environment(default_env) {
                info!("Using environment '{}' for legacy compatibility", default_env);
                ConnectionConfig {
                    database_url: format!(
                        "mysql://{}:{}@{}:{}/{}",
                        env_config.database.username,
                        env_config.database.password,
                        env_config.database.host,
                        env_config.database.port,
                        env_config.database.database
                    ),
                }
            } else {
                return Err(ServerError::configuration_error(
                    "default_environment".to_string(),
                    "Default environment configuration not found".to_string()
                ));
            }
        } else {
            // If no default environment is configured, use the first healthy environment
            let healthy_envs = pool_manager.get_healthy_environments().await;
            if let Some(first_healthy) = healthy_envs.first() {
                if let Some(env_config) = environment_manager.get_environment(first_healthy) {
                    info!("Using first healthy environment '{}' for legacy compatibility", first_healthy);
                    ConnectionConfig {
                        database_url: format!(
                            "mysql://{}:{}@{}:{}/{}",
                            env_config.database.username,
                            env_config.database.password,
                            env_config.database.host,
                            env_config.database.port,
                            env_config.database.database
                        ),
                    }
                } else {
                    return Err(ServerError::configuration_error(
                        "environments".to_string(),
                        "No healthy environment configuration found for legacy compatibility".to_string()
                    ));
                }
            } else {
                return Err(ServerError::configuration_error(
                    "environments".to_string(),
                    "No healthy environments available for legacy compatibility".to_string()
                ));
            }
        };

        let connection_manager = Arc::new(Mutex::new(ConnectionManager::new(legacy_config.clone())));
        let result_streamer = ResultStreamer::new();
        
        info!("Multi-environment MCP server created successfully");
        
        Ok(Self {
            config: legacy_config,
            connection_manager,
            result_streamer,
            environment_manager: Some(environment_manager),
            pool_manager: Some(pool_manager),
            query_router: Some(query_router),
            enhanced_mcp_tools: Some(enhanced_mcp_tools),
        })
    }

    /// Get the server configuration
    pub fn config(&self) -> &ConnectionConfig {
        &self.config
    }

    /// Initialize the database connection(s)
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing MySQL MCP server");
        
        if self.is_multi_environment() {
            // Multi-environment initialization
            self.initialize_multi_environment().await?;
        } else {
            // Legacy single database initialization
            self.initialize_legacy_database().await?;
        }
        
        info!("MySQL MCP server initialized successfully");
        Ok(())
    }

    /// Initialize legacy single database connection
    async fn initialize_legacy_database(&self) -> Result<()> {
        info!("Initializing legacy single database connection");
        
        let mut manager = self.connection_manager.lock().await;
        manager.connect().await?;
        manager.test_connection().await?;
        
        info!("Legacy database connection initialized successfully");
        Ok(())
    }

    /// Initialize multi-environment connections with graceful startup
    async fn initialize_multi_environment(&self) -> Result<()> {
        info!("Initializing multi-environment connections");
        
        let _environment_manager = self.environment_manager.as_ref()
            .ok_or_else(|| ServerError::internal_error(
                "Environment manager not available".to_string(),
                Some("Server was not initialized with multi-environment support".to_string())
            ))?;

        let _pool_manager = self.pool_manager.as_ref()
            .ok_or_else(|| ServerError::internal_error(
                "Pool manager not available".to_string(),
                Some("Server was not initialized with multi-environment support".to_string())
            ))?;

        // Perform startup environment validation
        self.validate_environments_at_startup().await?;

        // Perform startup health checks for all environments
        self.perform_startup_health_checks().await?;

        // Validate that at least one environment is healthy
        self.ensure_minimum_healthy_environments().await?;

        // Initialize legacy connection manager for backward compatibility
        info!("Initializing legacy connection manager for backward compatibility");
        let mut manager = self.connection_manager.lock().await;
        manager.connect().await?;
        manager.test_connection().await?;
        info!("Legacy connection manager initialized successfully");

        info!("Multi-environment initialization completed successfully");
        Ok(())
    }

    /// Validate all environments during startup
    async fn validate_environments_at_startup(&self) -> Result<()> {
        info!("Validating environments at startup");
        
        let environment_manager = self.environment_manager.as_ref().unwrap();
        let enabled_environments = environment_manager.list_enabled_environments();
        
        if enabled_environments.is_empty() {
            return Err(ServerError::configuration_error(
                "environments".to_string(),
                "No enabled environments found during startup validation".to_string()
            ));
        }

        let mut validation_errors = Vec::new();
        
        for env_name in &enabled_environments {
            match environment_manager.validate_environment(env_name) {
                Ok(()) => {
                    debug!("Environment '{}' validation passed", env_name);
                }
                Err(e) => {
                    let error_msg = format!("Environment '{}' validation failed: {}", env_name, e.user_message());
                    warn!("{}", error_msg);
                    validation_errors.push(error_msg);
                }
            }
        }

        if !validation_errors.is_empty() {
            return Err(ServerError::configuration_error(
                "environments".to_string(),
                format!("Environment validation failed during startup: {}", validation_errors.join("; "))
            ));
        }

        info!("All {} enabled environments passed startup validation", enabled_environments.len());
        Ok(())
    }

    /// Perform startup health checks for all configured environments
    async fn perform_startup_health_checks(&self) -> Result<()> {
        info!("Performing startup health checks for all environments");
        
        let pool_manager = self.pool_manager.as_ref().unwrap();
        let environment_manager = self.environment_manager.as_ref().unwrap();
        let enabled_environments = environment_manager.list_enabled_environments();
        
        let mut health_check_results = Vec::new();
        
        for env_name in &enabled_environments {
            info!("Performing startup health check for environment '{}'", env_name);
            
            match pool_manager.test_connection_simple(env_name).await {
                Ok(()) => {
                    info!("✓ Environment '{}' health check passed", env_name);
                    health_check_results.push((env_name.to_string(), true, None));
                }
                Err(e) => {
                    let error_msg = e.user_message();
                    warn!("✗ Environment '{}' health check failed: {}", env_name, error_msg);
                    health_check_results.push((env_name.to_string(), false, Some(error_msg)));
                }
            }
        }

        // Log summary of health check results
        let healthy_count = health_check_results.iter().filter(|(_, healthy, _)| *healthy).count();
        let total_count = health_check_results.len();
        
        info!("Startup health check summary: {}/{} environments healthy", healthy_count, total_count);
        
        // Log details for failed health checks
        for (env_name, healthy, error) in &health_check_results {
            if !healthy {
                if let Some(error_msg) = error {
                    warn!("Environment '{}' is unhealthy: {}", env_name, error_msg);
                }
            }
        }

        Ok(())
    }

    /// Ensure at least one environment is healthy for graceful startup
    async fn ensure_minimum_healthy_environments(&self) -> Result<()> {
        let pool_manager = self.pool_manager.as_ref().unwrap();
        let environment_manager = self.environment_manager.as_ref().unwrap();
        let enabled_environments = environment_manager.list_enabled_environments();
        
        let mut healthy_environments = Vec::new();
        
        for env_name in &enabled_environments {
            if pool_manager.is_environment_healthy(env_name).await {
                healthy_environments.push(*env_name);
            }
        }

        if healthy_environments.is_empty() {
            return Err(ServerError::connection_error(
                sqlx::Error::PoolClosed,
                false
            ).with_context("startup_validation", 
                "No healthy environments available. Server cannot start without at least one healthy database connection."));
        }

        if healthy_environments.len() < enabled_environments.len() {
            let unhealthy_count = enabled_environments.len() - healthy_environments.len();
            warn!("Starting with partial environment availability: {} healthy, {} unhealthy environments", 
                  healthy_environments.len(), unhealthy_count);
            
            // Log which environments are healthy vs unhealthy
            info!("Healthy environments: {}", healthy_environments.join(", "));
            
            let unhealthy_environments: Vec<&str> = enabled_environments.iter()
                .filter(|env| !healthy_environments.contains(env))
                .copied()
                .collect();
            
            if !unhealthy_environments.is_empty() {
                warn!("Unhealthy environments: {}", unhealthy_environments.join(", "));
            }
        } else {
            info!("All {} environments are healthy and ready", healthy_environments.len());
        }

        Ok(())
    }

    /// Check if this server instance supports multi-environment operations
    pub fn is_multi_environment(&self) -> bool {
        self.environment_manager.is_some() && 
        self.pool_manager.is_some() && 
        self.query_router.is_some() && 
        self.enhanced_mcp_tools.is_some()
    }

    /// Attempt graceful startup with partial environment failures
    async fn attempt_graceful_startup(&self) -> Result<()> {
        info!("Attempting graceful startup with partial environment availability");
        
        let pool_manager = self.pool_manager.as_ref()
            .ok_or_else(|| ServerError::internal_error(
                "Pool manager not available for graceful startup".to_string(),
                None
            ))?;

        let environment_manager = self.environment_manager.as_ref()
            .ok_or_else(|| ServerError::internal_error(
                "Environment manager not available for graceful startup".to_string(),
                None
            ))?;

        let enabled_environments = environment_manager.list_enabled_environments();
        let mut healthy_environments = Vec::new();

        // Check which environments are actually healthy
        for env_name in &enabled_environments {
            if pool_manager.is_environment_healthy(env_name).await {
                healthy_environments.push(*env_name);
            }
        }

        // Require at least one healthy environment for graceful startup
        if healthy_environments.is_empty() {
            return Err(ServerError::connection_error(
                sqlx::Error::PoolClosed,
                false
            ).with_context("graceful_startup", 
                "Cannot start server: no healthy environments available"));
        }

        // Log graceful startup status
        let unhealthy_count = enabled_environments.len() - healthy_environments.len();
        
        info!("Graceful startup successful with {} healthy environments", healthy_environments.len());
        info!("Healthy environments: {}", healthy_environments.join(", "));
        
        if unhealthy_count > 0 {
            let unhealthy_environments: Vec<&str> = enabled_environments.iter()
                .filter(|env| !healthy_environments.contains(env))
                .copied()
                .collect();
            
            warn!("Unhealthy environments ({}): {}", unhealthy_count, unhealthy_environments.join(", "));
            warn!("These environments will be unavailable until they recover");
        }

        Ok(())
    }

    /// Start the MCP server with HTTP transport
    pub async fn start(&self, port: u16) -> Result<()> {
        info!("Starting MCP server on port {}", port);

        // Initialize the connection(s) first with graceful startup handling
        match self.initialize().await {
            Ok(()) => {
                info!("Server initialization completed successfully");
            }
            Err(e) => {
                error!("Server initialization failed: {}", e.user_message());
                
                // For multi-environment setups, check if we can start with partial functionality
                if self.is_multi_environment() {
                    match self.attempt_graceful_startup().await {
                        Ok(()) => {
                            warn!("Server started with partial environment availability");
                        }
                        Err(graceful_err) => {
                            error!("Graceful startup also failed: {}", graceful_err.user_message());
                            return Err(e); // Return original error
                        }
                    }
                } else {
                    return Err(e);
                }
            }
        }

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
                let _server = server_for_get.clone();
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

        // Health check endpoint for Docker
        let server_for_health = server.clone();
        let health_route = warp::path("health")
            .and(warp::get())
            .and_then(move || {
                let server = server_for_health.clone();
                async move {
                    // Test database connection for health check
                    match server.test_database_health().await {
                        Ok(_) => {
                            let response = json!({
                                "status": "healthy",
                                "timestamp": chrono::Utc::now().to_rfc3339(),
                                "service": "mysql-mcp-server",
                                "version": "0.1.0"
                            });
                            Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&response),
                                warp::http::StatusCode::OK
                            ))
                        }
                        Err(e) => {
                            error!("Health check failed: {}", e);
                            let response = json!({
                                "status": "unhealthy",
                                "error": e.to_string(),
                                "timestamp": chrono::Utc::now().to_rfc3339(),
                                "service": "mysql-mcp-server",
                                "version": "0.1.0"
                            });
                            Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&response),
                                warp::http::StatusCode::SERVICE_UNAVAILABLE
                            ))
                        }
                    }
                }
            });

        let cors = warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type", "authorization", "x-requested-with", "accept"])
            .allow_methods(vec!["POST", "GET", "OPTIONS"])
            .expose_headers(vec!["content-type"]);

        let routes = mcp_route
            .or(mcp_get_route)
            .or(mcp_options)
            .or(stream_route)
            .or(health_route)
            .with(cors);

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
        
        let mut tools = vec![
            json!({
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
            }),
            json!({
                "name": "test_connection",
                "description": "Test the database connection",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            json!({
                "name": "list_databases",
                "description": "List all available databases",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            json!({
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
            }),
            json!({
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
            }),
            json!({
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
            })
        ];

        // Add enhanced multi-environment tools if available
        if let Some(enhanced_tools) = &self.enhanced_mcp_tools {
            let enhanced_tool_definitions = enhanced_tools.get_tool_definitions();
            if let Some(enhanced_tools_array) = enhanced_tool_definitions.get("tools").and_then(|v| v.as_array()) {
                tools.extend(enhanced_tools_array.iter().cloned());
            }
        }
        
        Ok(json!({
            "tools": tools
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
            // Enhanced multi-environment tools
            "execute_query_env" | "execute_query_multi_env" | "list_environments" | 
            "list_databases_env" | "list_databases_all_env" | "list_tables_env" | 
            "describe_table_env" | "compare_schema" | "health_check_env" | "test_connection_env" => {
                if let Some(enhanced_tools) = &self.enhanced_mcp_tools {
                    enhanced_tools.handle_tool_call(tool_name, arguments).await?
                } else {
                    return Err(ServerError::validation_error(
                        format!("Multi-environment tool '{}' is not available. Server was not initialized with multi-environment support.", tool_name),
                        Some(tool_name.to_string())
                    ));
                }
            },
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

    /// Test database health for health check endpoint
    pub async fn test_database_health(&self) -> Result<()> {
        // Check if we're in multi-environment mode
        if let Some(pool_manager) = &self.pool_manager {
            // Multi-environment mode: check at least one environment is healthy
            let environment_manager = self.environment_manager.as_ref().unwrap();
            let enabled_environments = environment_manager.list_enabled_environments();
            
            if enabled_environments.is_empty() {
                return Err(ServerError::validation_error(
                    "No enabled environments found".to_string(),
                    None
                ));
            }
            
            // Test connection for the default environment
            let default_env = environment_manager.get_default_environment()
                .ok_or_else(|| ServerError::validation_error(
                    "No default environment configured for health check".to_string(),
                    None
                ))?;
            pool_manager.test_connection_simple(default_env).await
        } else {
            // Legacy single-environment mode
            let mut manager = self.connection_manager.lock().await;
            manager.test_connection().await
        }
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
            environment_manager: self.environment_manager.clone(),
            pool_manager: self.pool_manager.clone(),
            query_router: self.query_router.clone(),
            enhanced_mcp_tools: self.enhanced_mcp_tools.clone(),
        }
    }
}

impl warp::reject::Reject for ServerError {}