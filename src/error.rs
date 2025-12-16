//! Error types for the MySQL MCP server

use std::fmt;
use tracing::{error, warn};

/// Result type alias for the server
pub type Result<T> = std::result::Result<T, ServerError>;

/// Categories of environment-specific errors
#[derive(Debug, Clone, PartialEq)]
pub enum EnvironmentErrorCategory {
    /// Environment configuration issues
    Configuration,
    /// Environment connectivity problems
    Connectivity,
    /// Environment authentication/authorization issues
    Authentication,
    /// Environment performance issues
    Performance,
    /// Environment resource exhaustion
    ResourceExhaustion,
    /// Environment not found or disabled
    Unavailable,
    /// Environment data inconsistency
    DataInconsistency,
}

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
        /// Environment context where the error occurred
        environment: Option<String>,
    },
    /// Query execution errors
    Query {
        /// The SQL query that failed
        sql: String,
        /// The underlying database error
        source: sqlx::Error,
        /// MySQL error code if available
        error_code: Option<String>,
        /// Environment context where the error occurred
        environment: Option<String>,
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
        /// Environment context where the error occurred
        environment: Option<String>,
    },
    /// Multi-environment operation errors
    MultiEnvironment {
        /// Operation that failed
        operation: String,
        /// Errors from individual environments
        environment_errors: std::collections::HashMap<String, Box<ServerError>>,
        /// Environments that succeeded
        successful_environments: Vec<String>,
        /// Overall operation result
        partial_success: bool,
    },
    /// Environment-specific errors
    Environment {
        /// Environment name
        environment: String,
        /// Environment-specific error message
        message: String,
        /// Underlying error if any
        source: Option<Box<ServerError>>,
        /// Error category
        category: EnvironmentErrorCategory,
    },
}

impl ServerError {
    /// Create a new connection error
    pub fn connection_error(source: sqlx::Error, recoverable: bool) -> Self {
        Self::connection_error_with_env(source, recoverable, None)
    }

    /// Create a new connection error with environment context
    pub fn connection_error_with_env(source: sqlx::Error, recoverable: bool, environment: Option<String>) -> Self {
        let message = Self::format_connection_error(&source, recoverable);
        
        if let Some(ref env) = environment {
            error!("Connection error in environment '{}': {} (recoverable: {})", env, source, recoverable);
        } else {
            error!("Connection error: {} (recoverable: {})", source, recoverable);
        }
        
        Self::Connection {
            source,
            message,
            recoverable,
            environment,
        }
    }

    /// Create a new query error
    pub fn query_error(sql: String, source: sqlx::Error) -> Self {
        Self::query_error_with_env(sql, source, None)
    }

    /// Create a new query error with environment context
    pub fn query_error_with_env(sql: String, source: sqlx::Error, environment: Option<String>) -> Self {
        let error_code = Self::extract_mysql_error_code(&source);
        let sanitized_sql = Self::sanitize_sql(&sql);
        
        if let Some(ref env) = environment {
            error!("Query execution failed in environment '{}': {} | SQL: {}", env, source, sanitized_sql);
        } else {
            error!("Query execution failed: {} | SQL: {}", source, sanitized_sql);
        }
        
        Self::Query {
            sql,
            source,
            error_code,
            environment,
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
        Self::internal_error_with_env(message, details, None)
    }

    /// Create a new internal error with environment context
    pub fn internal_error_with_env(message: String, details: Option<String>, environment: Option<String>) -> Self {
        if let Some(ref env) = environment {
            error!("Internal server error in environment '{}': {} | Details: {:?}", env, message, details);
        } else {
            error!("Internal server error: {} | Details: {:?}", message, details);
        }
        
        Self::Internal {
            message,
            details,
            environment,
        }
    }

    /// Create a new multi-environment error
    pub fn multi_environment_error(
        operation: String,
        environment_errors: std::collections::HashMap<String, Box<ServerError>>,
        successful_environments: Vec<String>,
    ) -> Self {
        let partial_success = !successful_environments.is_empty();
        let total_envs = environment_errors.len() + successful_environments.len();
        
        error!(
            "Multi-environment operation '{}' completed with {} successes and {} failures out of {} environments",
            operation, successful_environments.len(), environment_errors.len(), total_envs
        );
        
        // Log individual environment errors
        for (env, error) in &environment_errors {
            error!("Environment '{}' failed: {}", env, error.user_message());
        }
        
        Self::MultiEnvironment {
            operation,
            environment_errors,
            successful_environments,
            partial_success,
        }
    }

    /// Create a new environment-specific error
    pub fn environment_error(
        environment: String,
        message: String,
        source: Option<Box<ServerError>>,
        category: EnvironmentErrorCategory,
    ) -> Self {
        error!(
            "Environment '{}' error ({:?}): {} | Source: {:?}",
            environment, category, message, source.as_ref().map(|e| e.user_message())
        );
        
        Self::Environment {
            environment,
            message,
            source,
            category,
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
            ServerError::MultiEnvironment { partial_success, .. } => *partial_success, // Recoverable if some environments succeeded
            ServerError::Environment { category, source, .. } => {
                match category {
                    EnvironmentErrorCategory::Connectivity => true,
                    EnvironmentErrorCategory::Performance => true,
                    EnvironmentErrorCategory::ResourceExhaustion => true,
                    EnvironmentErrorCategory::Configuration => false,
                    EnvironmentErrorCategory::Authentication => false,
                    EnvironmentErrorCategory::Unavailable => false,
                    EnvironmentErrorCategory::DataInconsistency => false,
                }
                .then_some(true)
                .or_else(|| source.as_ref().map(|s| s.is_recoverable()))
                .unwrap_or(false)
            }
        }
    }

    /// Get the environment context for this error
    pub fn environment(&self) -> Option<&str> {
        match self {
            ServerError::Connection { environment, .. } => environment.as_deref(),
            ServerError::Query { environment, .. } => environment.as_deref(),
            ServerError::Internal { environment, .. } => environment.as_deref(),
            ServerError::Environment { environment, .. } => Some(environment),
            _ => None,
        }
    }

    /// Check if this is a multi-environment error
    pub fn is_multi_environment(&self) -> bool {
        matches!(self, ServerError::MultiEnvironment { .. })
    }

    /// Get the list of affected environments
    pub fn affected_environments(&self) -> Vec<String> {
        match self {
            ServerError::MultiEnvironment { environment_errors, successful_environments, .. } => {
                let mut envs = successful_environments.clone();
                envs.extend(environment_errors.keys().cloned());
                envs
            }
            ServerError::Environment { environment, .. } => vec![environment.clone()],
            _ => {
                if let Some(env) = self.environment() {
                    vec![env.to_string()]
                } else {
                    vec![]
                }
            }
        }
    }

    /// Get a user-friendly error message (safe to send to clients)
    pub fn user_message(&self) -> String {
        match self {
            ServerError::Connection { message, environment, .. } => {
                if let Some(env) = environment {
                    format!("Connection error in environment '{}': {}", env, message)
                } else {
                    message.clone()
                }
            }
            ServerError::Query { source, environment, .. } => {
                let base_msg = format!("Query execution failed: {}", Self::sanitize_database_error(source));
                if let Some(env) = environment {
                    format!("{} (environment: {})", base_msg, env)
                } else {
                    base_msg
                }
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
            ServerError::Internal { message, environment, .. } => {
                if let Some(env) = environment {
                    format!("Internal error in environment '{}': {}", env, message)
                } else {
                    message.clone()
                }
            }
            ServerError::MultiEnvironment { operation, environment_errors, successful_environments, partial_success } => {
                let total_envs = environment_errors.len() + successful_environments.len();
                if *partial_success {
                    format!(
                        "Multi-environment operation '{}' partially succeeded: {}/{} environments completed successfully",
                        operation, successful_environments.len(), total_envs
                    )
                } else {
                    format!(
                        "Multi-environment operation '{}' failed: all {} environments encountered errors",
                        operation, environment_errors.len()
                    )
                }
            }
            ServerError::Environment { environment, message, category, .. } => {
                format!("Environment '{}' error ({:?}): {}", environment, category, message)
            }
        }
    }

    /// Get detailed error information for logging
    pub fn detailed_message(&self) -> String {
        match self {
            ServerError::Connection { source, message, recoverable, environment } => {
                format!(
                    "Connection error: {} | Recoverable: {} | Environment: {:?} | Source: {}",
                    message, recoverable, environment, source
                )
            }
            ServerError::Query { sql, source, error_code, environment } => {
                let sanitized_sql = Self::sanitize_sql(sql);
                format!(
                    "Query error: {} | SQL: {} | Error code: {:?} | Environment: {:?}",
                    source, sanitized_sql, error_code, environment
                )
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
            ServerError::Internal { message, details, environment } => {
                format!(
                    "Internal error: {} | Details: {:?} | Environment: {:?}",
                    message, details, environment
                )
            }
            ServerError::MultiEnvironment { operation, environment_errors, successful_environments, partial_success } => {
                let mut details = format!(
                    "Multi-environment operation '{}' | Partial success: {} | Successful: {:?}",
                    operation, partial_success, successful_environments
                );
                
                if !environment_errors.is_empty() {
                    details.push_str(" | Failed environments: ");
                    for (env, error) in environment_errors {
                        details.push_str(&format!("{}({}), ", env, error.user_message()));
                    }
                }
                
                details
            }
            ServerError::Environment { environment, message, source, category } => {
                format!(
                    "Environment '{}' error ({:?}): {} | Source: {:?}",
                    environment, category, message, source.as_ref().map(|e| e.user_message())
                )
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
            .replace("pwd=", "pwd=[REDACTED]")
            .replace("user=", "user=[REDACTED]")
            .replace("username=", "username=[REDACTED]")
            .replace("host=", "host=[REDACTED]");
        
        // Limit error message length to prevent log flooding
        if sanitized.len() > 500 {
            format!("{}...", &sanitized[..497])
        } else {
            sanitized
        }
    }

    /// Sanitize SQL queries to remove sensitive information for logging
    fn sanitize_sql(sql: &str) -> String {
        // Remove potential sensitive data from SQL queries
        let sanitized = sql
            .replace("password", "[REDACTED]")
            .replace("PASSWORD", "[REDACTED]")
            .replace("secret", "[REDACTED]")
            .replace("SECRET", "[REDACTED]")
            .replace("token", "[REDACTED]")
            .replace("TOKEN", "[REDACTED]");
        
        // Limit SQL length for logging
        if sanitized.len() > 200 {
            format!("{}...", &sanitized[..197])
        } else {
            sanitized
        }
    }

    /// Create a structured error response for multi-environment operations
    pub fn to_structured_response(&self) -> serde_json::Value {
        use serde_json::json;
        
        match self {
            ServerError::MultiEnvironment { operation, environment_errors, successful_environments, partial_success } => {
                let mut error_details = serde_json::Map::new();
                
                for (env, error) in environment_errors {
                    error_details.insert(env.clone(), json!({
                        "error": error.user_message(),
                        "recoverable": error.is_recoverable(),
                        "category": "environment_error"
                    }));
                }
                
                json!({
                    "error_type": "multi_environment",
                    "operation": operation,
                    "partial_success": partial_success,
                    "successful_environments": successful_environments,
                    "failed_environments": error_details,
                    "summary": {
                        "total_environments": environment_errors.len() + successful_environments.len(),
                        "successful_count": successful_environments.len(),
                        "failed_count": environment_errors.len()
                    },
                    "message": self.user_message(),
                    "recoverable": self.is_recoverable()
                })
            }
            ServerError::Environment { environment, message, category, .. } => {
                json!({
                    "error_type": "environment_specific",
                    "environment": environment,
                    "category": format!("{:?}", category),
                    "message": message,
                    "recoverable": self.is_recoverable()
                })
            }
            _ => {
                json!({
                    "error_type": "standard",
                    "message": self.user_message(),
                    "recoverable": self.is_recoverable(),
                    "environment": self.environment()
                })
            }
        }
    }

    /// Add context to an existing error
    pub fn with_context(mut self, context_type: &str, context_message: &str) -> Self {
        match &mut self {
            ServerError::Connection { message, .. } => {
                *message = format!("{}: {}", context_type, context_message);
            }
            ServerError::Query { .. } => {
                // For query errors, we don't modify the message as it contains important SQL info
            }
            ServerError::Validation { message, .. } => {
                *message = format!("{}: {}", context_type, context_message);
            }
            ServerError::Configuration { message, .. } => {
                *message = format!("{}: {}", context_type, context_message);
            }
            ServerError::Internal { message, .. } => {
                *message = format!("{}: {}", context_type, context_message);
            }
            _ => {
                // For other error types, we don't modify the message
            }
        }
        self
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

impl Clone for ServerError {
    fn clone(&self) -> Self {
        match self {
            ServerError::Connection { message, environment, .. } => {
                ServerError::Internal {
                    message: format!("Connection error: {}", message),
                    details: Some("Cloned from connection error".to_string()),
                    environment: environment.clone(),
                }
            }
            ServerError::Query { sql, environment, .. } => {
                ServerError::Internal {
                    message: format!("Query error: {}", Self::sanitize_sql(sql)),
                    details: Some("Cloned from query error".to_string()),
                    environment: environment.clone(),
                }
            }
            ServerError::Validation { message, invalid_value } => {
                ServerError::Validation {
                    message: message.clone(),
                    invalid_value: invalid_value.clone(),
                }
            }
            ServerError::Serialization { context, .. } => {
                ServerError::Internal {
                    message: format!("Serialization error in {}", context),
                    details: Some("Cloned from serialization error".to_string()),
                    environment: None,
                }
            }
            ServerError::ResourceExhaustion { resource_type, usage_info } => {
                ServerError::ResourceExhaustion {
                    resource_type: resource_type.clone(),
                    usage_info: usage_info.clone(),
                }
            }
            ServerError::Timeout { operation, timeout_ms } => {
                ServerError::Timeout {
                    operation: operation.clone(),
                    timeout_ms: *timeout_ms,
                }
            }
            ServerError::Configuration { parameter, message } => {
                ServerError::Configuration {
                    parameter: parameter.clone(),
                    message: message.clone(),
                }
            }
            ServerError::Io { context, .. } => {
                ServerError::Internal {
                    message: format!("I/O error in {}", context),
                    details: Some("Cloned from I/O error".to_string()),
                    environment: None,
                }
            }
            ServerError::Protocol { message, request_id } => {
                ServerError::Protocol {
                    message: message.clone(),
                    request_id: request_id.clone(),
                }
            }
            ServerError::Internal { message, details, environment } => {
                ServerError::Internal {
                    message: message.clone(),
                    details: details.clone(),
                    environment: environment.clone(),
                }
            }
            ServerError::MultiEnvironment { operation, environment_errors, successful_environments, partial_success } => {
                ServerError::MultiEnvironment {
                    operation: operation.clone(),
                    environment_errors: environment_errors.clone(),
                    successful_environments: successful_environments.clone(),
                    partial_success: *partial_success,
                }
            }
            ServerError::Environment { environment, message, source, category } => {
                ServerError::Environment {
                    environment: environment.clone(),
                    message: message.clone(),
                    source: source.clone(),
                    category: category.clone(),
                }
            }
        }
    }
}

/// Secure logging utilities for multi-environment operations
pub mod secure_logging {
    use tracing::{info, warn, error, debug};
    use serde_json::Value;
    use std::collections::HashMap;

    /// Log a multi-environment operation with secure context
    pub fn log_multi_env_operation(
        operation: &str,
        environments: &[String],
        level: LogLevel,
        additional_context: Option<&HashMap<String, Value>>,
    ) {
        let env_count = environments.len();
        let env_list = if env_count <= 5 {
            environments.join(", ")
        } else {
            format!("{} environments ({}...)", env_count, environments[..3].join(", "))
        };

        let context_str = additional_context
            .map(|ctx| sanitize_log_context(ctx))
            .unwrap_or_default();

        match level {
            LogLevel::Debug => debug!(
                "Multi-env operation '{}' on [{}] | Context: {}",
                operation, env_list, context_str
            ),
            LogLevel::Info => info!(
                "Multi-env operation '{}' on [{}] | Context: {}",
                operation, env_list, context_str
            ),
            LogLevel::Warn => warn!(
                "Multi-env operation '{}' on [{}] | Context: {}",
                operation, env_list, context_str
            ),
            LogLevel::Error => error!(
                "Multi-env operation '{}' on [{}] | Context: {}",
                operation, env_list, context_str
            ),
        }
    }

    /// Log environment-specific operation with secure context
    pub fn log_env_operation(
        environment: &str,
        operation: &str,
        level: LogLevel,
        duration_ms: Option<u64>,
        additional_context: Option<&HashMap<String, Value>>,
    ) {
        let duration_str = duration_ms
            .map(|d| format!(" ({}ms)", d))
            .unwrap_or_default();

        let context_str = additional_context
            .map(|ctx| sanitize_log_context(ctx))
            .unwrap_or_default();

        match level {
            LogLevel::Debug => debug!(
                "Environment '{}': {}{} | Context: {}",
                environment, operation, duration_str, context_str
            ),
            LogLevel::Info => info!(
                "Environment '{}': {}{} | Context: {}",
                environment, operation, duration_str, context_str
            ),
            LogLevel::Warn => warn!(
                "Environment '{}': {}{} | Context: {}",
                environment, operation, duration_str, context_str
            ),
            LogLevel::Error => error!(
                "Environment '{}': {}{} | Context: {}",
                environment, operation, duration_str, context_str
            ),
        }
    }

    /// Log query execution with environment context (sanitized)
    pub fn log_query_execution(
        environment: &str,
        sql: &str,
        success: bool,
        duration_ms: u64,
        affected_rows: Option<u64>,
    ) {
        let sanitized_sql = sanitize_sql_for_logging(sql);
        let rows_str = affected_rows
            .map(|r| format!(" | Rows: {}", r))
            .unwrap_or_default();

        if success {
            info!(
                "Query executed in '{}' ({}ms){} | SQL: {}",
                environment, duration_ms, rows_str, sanitized_sql
            );
        } else {
            error!(
                "Query failed in '{}' ({}ms) | SQL: {}",
                environment, duration_ms, sanitized_sql
            );
        }
    }

    /// Log connection events with environment context
    pub fn log_connection_event(
        environment: &str,
        event: ConnectionEvent,
        additional_info: Option<&str>,
    ) {
        let info_str = additional_info
            .map(|info| format!(" | {}", info))
            .unwrap_or_default();

        match event {
            ConnectionEvent::Acquired => debug!(
                "Connection acquired for environment '{}'{}", environment, info_str
            ),
            ConnectionEvent::Released => debug!(
                "Connection released for environment '{}'{}", environment, info_str
            ),
            ConnectionEvent::Failed => error!(
                "Connection failed for environment '{}'{}", environment, info_str
            ),
            ConnectionEvent::HealthCheck => debug!(
                "Health check for environment '{}'{}", environment, info_str
            ),
            ConnectionEvent::Reconnect => info!(
                "Reconnecting to environment '{}'{}", environment, info_str
            ),
        }
    }

    /// Sanitize log context to remove sensitive information
    fn sanitize_log_context(context: &HashMap<String, Value>) -> String {
        let mut sanitized = HashMap::new();
        
        for (key, value) in context {
            let key_lower = key.to_lowercase();
            if key_lower.contains("password") || 
               key_lower.contains("secret") || 
               key_lower.contains("token") || 
               key_lower.contains("credential") {
                sanitized.insert(key.clone(), Value::String("[REDACTED]".to_string()));
            } else {
                sanitized.insert(key.clone(), value.clone());
            }
        }
        
        serde_json::to_string(&sanitized).unwrap_or_else(|_| "{}".to_string())
    }

    /// Sanitize SQL for logging
    fn sanitize_sql_for_logging(sql: &str) -> String {
        // Remove potential sensitive data and limit length
        let sanitized = sql
            .replace("password", "[REDACTED]")
            .replace("PASSWORD", "[REDACTED]")
            .replace("secret", "[REDACTED]")
            .replace("SECRET", "[REDACTED]")
            .replace("token", "[REDACTED]")
            .replace("TOKEN", "[REDACTED]");
        
        if sanitized.len() > 150 {
            format!("{}...", &sanitized[..147])
        } else {
            sanitized
        }
    }

    /// Log levels for secure logging
    #[derive(Debug, Clone, Copy)]
    pub enum LogLevel {
        Debug,
        Info,
        Warn,
        Error,
    }

    /// Connection events for logging
    #[derive(Debug, Clone, Copy)]
    pub enum ConnectionEvent {
        Acquired,
        Released,
        Failed,
        HealthCheck,
        Reconnect,
    }
}