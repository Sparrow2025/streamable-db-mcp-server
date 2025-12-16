//! MySQL MCP Server
//! 
//! A Streamable MySQL MCP (Model Context Protocol) server implementation in Rust
//! that provides database connectivity and query execution capabilities.

pub mod config;
pub mod connection;
pub mod environment;
pub mod mcp_tools;
pub mod pool;
pub mod query;
pub mod router;
pub mod server;
pub mod streaming;
pub mod error;

pub use config::{Config, ConnectionConfig, ServerConfig, DatabaseConfig, EnvironmentConfig, PoolConfig, McpConfig};
pub use environment::{EnvironmentManager, EnvironmentMetadata, EnvironmentStatus, EnvironmentStatusReport};
pub use mcp_tools::{EnhancedMcpTools, SchemaComparisonResult, SchemaDifference, SchemaDifferenceType};
pub use pool::{ConnectionPoolManager, PoolHealthStatus, PoolInfo, PoolStats, ReconnectionState};
pub use router::{QueryRouter, EnvQueryRequest, MultiEnvQueryRequest, EnvQueryResponse, MultiEnvQueryResponse, ComparisonResult};
pub use error::{ServerError, Result};

// Re-export server module for external use
pub use server::McpServer;