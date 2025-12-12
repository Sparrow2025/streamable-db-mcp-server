//! Error types for the MySQL MCP server

use std::fmt;
use tracing::{error, warn};

/// Result type alias for the server
pub type Result<T> = std::result::Result<T, ServerError>;

/// Main error type for the MySQL MCP server
#[derive(Debug)]
pub enum ServerError {
    /// Database connection errors
    Connection {
        /// The underlying database error
        source: sqlx::Error,
        /// User-friendly error message
        message: String,
        /// Whether this is a recoverable error
        recoverable: bool,
    },
    /// Query execution errors
    Query {
        /// The SQL query that failed
        sql: String,
        /// The underlying database error
        source: sqlx::Error,
        /// MySQL error code if available
        error_code: Option<String>,
    },
    /// Connection validation errors
    Validation {
        /// The validation error message
        message: String,
        /// The invalid value that caused the error
        invalid_value: Option<String>,
    },
    /// Serialization/deserialization errors
    Serialization {
        /// The underlying serialization error
        source: serde_json::Error,
        /// Context about what was being serialized
        context: String,
    },
    /// Resource exhaustion errors
    ResourceExhaustion {
        /// Type of resource that was exhausted
        resource_type: String,
        /// Current usage information
        usage_info: Option<String>,
    },
    /// Timeout errors
    Timeout {
        /// Operation that timed out
        operation: String,
        /// Timeout duration in milliseconds
        timeout_ms: u64,
    },
    /// Configuration errors
    Configuration {
        /// Configuration parameter that is invalid
        parameter: String,
        /// Error message
        message: String,
    },
    /// General I/O errors
    Io {
        /// The underlying I/O error
        source: std::io::Error,
        /// Context about the I/O operation
        context: String,
    },
    /// Protocol errors (MCP-specific)
    Protocol {
        /// Protocol error message
        message: String,
        /// Request ID if available
        request_id: Option<String>,
    },
    /// Internal server errors
    Internal {
        /// Error message (safe for client)
        message: String,
        /// Internal error details (for logging only)
        details: Option<String>,
    },
}

impl ServerError {
    /// Create a new connection error
    pub fn connection_error(source: sqlx::Error, recoverable: bool) -> Self {
        let message = Self::format_connection_error(&source, recoverable);
        error!("Connection error: {} (recoverable: {})", source, recoverable);
        
        Self::Connection {
            source,
            message,
            recoverable,
        }
    }

    /// Create a new query error
    pub fn query_error(sql: String, source: sqlx::Error) -> Self {
        let error_code = Self::extract_mysql_error_code(&source);
        error!("Query execution failed: {} | SQL: {}", source, sql);
        
        Self::Query {
            sql,
            source,
            error_code,
        }
    }

    /// Create a new validation error
    pub fn validation_error(message: String, invalid_value: Option<String>) -> Self {
        warn!("Validation error: {} | Invalid value: {:?}", message, invalid_value);
        
        Self::Validation {
            message,
            invalid_value,
        }
    }

    /// Create a new serialization error
    pub fn serialization_error(source: serde_json::Error, context: String) -> Self {
        error!("Serialization error in {}: {}", context, source);
        
        Self::Serialization {
            source,
            context,
        }
    }

    /// Create a new resource exhaustion error
    pub fn resource_exhaustion(resource_type: String, usage_info: Option<String>) -> Self {
        error!("Resource exhaustion: {} | Usage: {:?}", resource_type, usage_info);
        
        Self::ResourceExhaustion {
            resource_type,
            usage_info,
        }
    }

    /// Create a new timeout error
    pub fn timeout_error(operation: String, timeout_ms: u64) -> Self {
        warn!("Operation timed out: {} after {}ms", operation, timeout_ms);
        
        Self::Timeout {
            operation,
            timeout_ms,
        }
    }

    /// Create a new configuration error
    pub fn configuration_error(parameter: String, message: String) -> Self {
        error!("Configuration error for '{}': {}", parameter, message);
        
        Self::Configuration {
            parameter,
            message,
        }
    }

    /// Create a new I/O error
    pub fn io_error(source: std::io::Error, context: String) -> Self {
        error!("I/O error in {}: {}", context, source);
        
        Self::Io {
            source,
            context,
        }
    }

    /// Create a new protocol error
    pub fn protocol_error(message: String, request_id: Option<String>) -> Self {
        warn!("Protocol error: {} | Request ID: {:?}", message, request_id);
        
        Self::Protocol {
            message,
            request_id,
        }
    }

    /// Create a new internal error
    pub fn internal_error(message: String, details: Option<String>) -> Self {
        error!("Internal server error: {} | Details: {:?}", message, details);
        
        Self::Internal {
            message,
            details,
        }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            ServerError::Connection { recoverable, .. } => *recoverable,
            ServerError::Query { .. } => false, // Query errors are generally not recoverable
            ServerError::Validation { .. } => false, // Validation errors require client fix
            ServerError::Serialization { .. } => false, // Serialization errors are not recoverable
            ServerError::ResourceExhaustion { .. } => true, // May recover when resources are freed
            ServerError::Timeout { .. } => true, // Timeouts may be recoverable with retry
            ServerError::Configuration { .. } => false, // Configuration errors require fix
            ServerError::Io { .. } => true, // I/O errors may be transient
            ServerError::Protocol { .. } => false, // Protocol errors require client fix
            ServerError::Internal { .. } => false, // Internal errors are not recoverable
        }
    }

    /// Get a user-friendly error message (safe to send to clients)
    pub fn user_message(&self) -> String {
        match self {
            ServerError::Connection { message, .. } => message.clone(),
            ServerError::Query { source, .. } => {
                format!("Query execution failed: {}", Self::sanitize_database_error(source))
            }
            ServerError::Validation { message, .. } => message.clone(),
            ServerError::Serialization { context, .. } => {
                format!("Data serialization error in {}", context)
            }
            ServerError::ResourceExhaustion { resource_type, .. } => {
                format!("Server resource exhaustion: {}", resource_type)
            }
            ServerError::Timeout { operation, timeout_ms } => {
                format!("Operation '{}' timed out after {}ms", operation, timeout_ms)
            }
            ServerError::Configuration { parameter, message } => {
                format!("Configuration error for '{}': {}", parameter, message)
            }
            ServerError::Io { context, .. } => {
                format!("I/O error during {}", context)
            }
            ServerError::Protocol { message, .. } => message.clone(),
            ServerError::Internal { message, .. } => message.clone(),
        }
    }

    /// Get detailed error information for logging
    pub fn detailed_message(&self) -> String {
        match self {
            ServerError::Connection { source, message, recoverable } => {
                format!("Connection error: {} | Recoverable: {} | Source: {}", message, recoverable, source)
            }
            ServerError::Query { sql, source, error_code } => {
                format!("Query error: {} | SQL: {} | Error code: {:?}", source, sql, error_code)
            }
            ServerError::Validation { message, invalid_value } => {
                format!("Validation error: {} | Invalid value: {:?}", message, invalid_value)
            }
            ServerError::Serialization { source, context } => {
                format!("Serialization error in {}: {}", context, source)
            }
            ServerError::ResourceExhaustion { resource_type, usage_info } => {
                format!("Resource exhaustion: {} | Usage: {:?}", resource_type, usage_info)
            }
            ServerError::Timeout { operation, timeout_ms } => {
                format!("Timeout: {} after {}ms", operation, timeout_ms)
            }
            ServerError::Configuration { parameter, message } => {
                format!("Configuration error for '{}': {}", parameter, message)
            }
            ServerError::Io { source, context } => {
                format!("I/O error in {}: {}", context, source)
            }
            ServerError::Protocol { message, request_id } => {
                format!("Protocol error: {} | Request ID: {:?}", message, request_id)
            }
            ServerError::Internal { message, details } => {
                format!("Internal error: {} | Details: {:?}", message, details)
            }
        }
    }

    /// Format connection error message based on the underlying error
    fn format_connection_error(source: &sqlx::Error, recoverable: bool) -> String {
        match source {
            sqlx::Error::Io(_) => {
                if recoverable {
                    "Unable to connect to database. Please check network connectivity and try again.".to_string()
                } else {
                    "Database connection failed due to network error.".to_string()
                }
            }
            sqlx::Error::Tls(_) => {
                "Database connection failed due to TLS/SSL error. Please check certificate configuration.".to_string()
            }
            sqlx::Error::Protocol(_) => {
                "Database connection failed due to protocol error. Please check database version compatibility.".to_string()
            }
            sqlx::Error::Configuration(_) => {
                "Database connection failed due to configuration error. Please check connection parameters.".to_string()
            }
            _ => {
                if source.to_string().contains("authentication") || source.to_string().contains("access denied") {
                    "Database connection failed: Invalid credentials or insufficient permissions.".to_string()
                } else if source.to_string().contains("timeout") {
                    "Database connection timed out. Please check network connectivity and database availability.".to_string()
                } else {
                    format!("Database connection failed: {}", Self::sanitize_database_error(source))
                }
            }
        }
    }

    /// Extract MySQL error code from sqlx error if available
    fn extract_mysql_error_code(source: &sqlx::Error) -> Option<String> {
        // Try to extract MySQL error code from the error message
        let error_str = source.to_string();
        
        // MySQL errors typically have format like "ERROR 1045 (28000): Access denied"
        if let Some(start) = error_str.find("ERROR ") {
            if let Some(end) = error_str[start + 6..].find(' ') {
                let code_str = &error_str[start + 6..start + 6 + end];
                return Some(code_str.to_string());
            }
        }
        
        // Alternative format: "(code: 1045)"
        if let Some(start) = error_str.find("(code: ") {
            if let Some(end) = error_str[start + 7..].find(')') {
                let code_str = &error_str[start + 7..start + 7 + end];
                return Some(code_str.to_string());
            }
        }
        
        None
    }

    /// Sanitize database error messages to remove sensitive information
    fn sanitize_database_error(source: &sqlx::Error) -> String {
        let error_str = source.to_string();
        
        // Remove potential sensitive information like connection strings, passwords, etc.
        let sanitized = error_str
            .replace(&std::env::var("DATABASE_URL").unwrap_or_default(), "[DATABASE_URL]")
            .replace("password=", "password=[REDACTED]")
            .replace("pwd=", "pwd=[REDACTED]");
        
        // Limit error message length to prevent log flooding
        if sanitized.len() > 500 {
            format!("{}...", &sanitized[..497])
        } else {
            sanitized
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.user_message())
    }
}

impl std::error::Error for ServerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ServerError::Connection { source, .. } => Some(source),
            ServerError::Query { source, .. } => Some(source),
            ServerError::Serialization { source, .. } => Some(source),
            ServerError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

// Conversion implementations for backward compatibility and convenience
impl From<sqlx::Error> for ServerError {
    fn from(err: sqlx::Error) -> Self {
        // Determine if the error is recoverable based on its type
        let recoverable = matches!(err, 
            sqlx::Error::Io(_) | 
            sqlx::Error::PoolTimedOut | 
            sqlx::Error::PoolClosed
        );
        
        ServerError::connection_error(err, recoverable)
    }
}

impl From<serde_json::Error> for ServerError {
    fn from(err: serde_json::Error) -> Self {
        ServerError::serialization_error(err, "unknown context".to_string())
    }
}

impl From<std::io::Error> for ServerError {
    fn from(err: std::io::Error) -> Self {
        ServerError::io_error(err, "unknown context".to_string())
    }
}