//! MySQL MCP Server
//! 
//! A Streamable MySQL MCP (Model Context Protocol) server implementation in Rust
//! that provides database connectivity and query execution capabilities.

pub mod config;
pub mod connection;
pub mod query;
pub mod server;
pub mod streaming;
pub mod error;

pub use config::{Config, ConnectionConfig, ServerConfig, DatabaseConfig, McpConfig};
pub use error::{ServerError, Result};

// Re-export server module for external use
pub use server::McpServer;