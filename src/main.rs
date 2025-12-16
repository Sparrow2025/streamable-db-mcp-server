use mysql_mcp_server::Result;
use mysql_mcp_server::config::Config;
use mysql_mcp_server::server::McpServer;
use tokio::signal;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration first to get the log level
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            // Initialize basic tracing for error reporting
            tracing_subscriber::fmt::init();
            error!("Failed to load configuration: {}", e.user_message());
            error!("Configuration error details: {}", e.detailed_message());
            return Err(e);
        }
    };

    // Initialize tracing with the configured log level
    let log_level = match config.server.log_level.to_lowercase().as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .init();

    info!("Starting MySQL MCP Server initialization");
    info!("Configuration loaded successfully from config file");
    info!("Log level set to: {}", config.server.log_level);
    info!("Server will listen on port: {}", config.server.port);
    
    // Log database configuration based on mode (legacy or multi-environment)
    if config.is_multi_environment() {
        info!("Multi-environment mode enabled");
        if let Some(default_env) = config.get_default_environment() {
            info!("Default environment: {}", default_env);
        }
        let enabled_envs = config.get_enabled_environments();
        info!("Enabled environments: {}", enabled_envs.join(", "));
        
        // Log details for each enabled environment
        if let Some(environments) = config.get_environments() {
            for (env_name, env_config) in environments {
                if env_config.enabled {
                    info!("Environment '{}': {}", env_name, env_config.masked_connection_url());
                    info!("Environment '{}' pool: max={}, min={}", 
                        env_name, 
                        env_config.connection_pool.max_connections,
                        env_config.connection_pool.min_connections
                    );
                }
            }
        }
    } else {
        // Legacy single database mode
        if let Some(db_config) = config.get_legacy_database() {
            info!("Legacy single database mode");
            info!("Database connection: {}", db_config.masked_connection_url());
            info!("Connection timeout: {}s", db_config.connection_timeout);
            info!("Max connections: {}", db_config.max_connections);
        }
    }

    // Create the MCP server based on configuration type
    let server = if config.is_multi_environment() {
        info!("Initializing server with multi-environment support");
        McpServer::with_multi_environment(config.clone()).await?
    } else {
        info!("Initializing server with legacy single-database configuration");
        // Create connection configuration for backward compatibility
        let connection_config = config.to_connection_config();
        McpServer::new(connection_config)
    };

    // Set up graceful shutdown handling
    let shutdown_signal = setup_shutdown_signal();

    info!("Starting MySQL MCP Server on port {}", config.server.port);

    // Clone server for shutdown handling
    let server_for_shutdown = server.clone();

    // Start the server with graceful shutdown
    tokio::select! {
        result = server.start(config.server.port) => {
            match result {
                Ok(_) => {
                    info!("Server stopped normally");
                    Ok(())
                }
                Err(e) => {
                    error!("Server failed: {}", e.user_message());
                    error!("Server error details: {}", e.detailed_message());
                    Err(e)
                }
            }
        }
        _ = shutdown_signal => {
            info!("Shutdown signal received, stopping server gracefully");
            
            // Perform graceful shutdown
            if let Err(e) = server_for_shutdown.shutdown().await {
                error!("Error during graceful shutdown: {}", e.user_message());
                // Continue with shutdown even if there are errors
            }
            
            info!("Server shutdown complete");
            Ok(())
        }
    }
}



/// Set up graceful shutdown signal handling
async fn setup_shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received terminate signal");
        },
    }
}