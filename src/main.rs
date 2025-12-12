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
    info!("Database connection: {}", config.database.masked_connection_url());
    info!("Connection timeout: {}s", config.database.connection_timeout);
    info!("Max connections: {}", config.database.max_connections);

    // Create connection configuration for backward compatibility
    let connection_config = config.to_connection_config();

    // Create the MCP server
    let server = McpServer::new(connection_config);

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