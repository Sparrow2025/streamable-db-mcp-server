//! Configuration structures for the MySQL MCP server

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::{Result, ServerError};

/// Complete server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<DatabaseConfig>, // Legacy single database config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environments: Option<HashMap<String, EnvironmentConfig>>, // New multi-environment config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_environment: Option<String>, // Default environment name
    pub mcp: McpConfig,
}

/// Server configuration section
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Server listening port
    pub port: u16,
    /// Log level (trace, debug, info, warn, error)
    pub log_level: String,
}

/// Database configuration section
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    /// Database host (e.g., localhost, 192.168.1.100)
    pub host: String,
    /// Database port (default: 3306)
    #[serde(default = "default_mysql_port")]
    pub port: u16,
    /// Database username
    pub username: String,
    /// Database password
    pub password: String,
    /// Database name
    pub database: String,
    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
    /// Maximum number of connections in the pool
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

/// Environment configuration for multi-database support
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnvironmentConfig {
    /// Environment name (e.g., "dev", "sit", "uat", "prod")
    pub name: String,
    /// Optional description of the environment
    pub description: Option<String>,
    /// Database connection configuration
    pub database: DatabaseConfig,
    /// Connection pool configuration
    pub connection_pool: PoolConfig,
    /// Whether this environment is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// Connection pool configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Minimum number of connections to maintain
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
    /// Idle timeout in seconds (how long to keep idle connections)
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,
}

impl DatabaseConfig {
    /// Build MySQL connection URL from individual components
    pub fn build_connection_url(&self) -> String {
        format!(
            "mysql://{}:{}@{}:{}/{}",
            self.username,
            self.password,
            self.host,
            self.port,
            self.database
        )
    }

    /// Get masked connection URL for logging (hides password)
    pub fn masked_connection_url(&self) -> String {
        format!(
            "mysql://{}:****@{}:{}/{}",
            self.username,
            self.host,
            self.port,
            self.database
        )
    }

    /// Validate the database configuration
    pub fn validate(&self) -> Result<()> {
        if self.host.is_empty() {
            return Err(ServerError::validation_error(
                "Database host cannot be empty".to_string(),
                Some("Provide a valid hostname or IP address".to_string())
            ));
        }

        if self.username.is_empty() {
            return Err(ServerError::validation_error(
                "Database username cannot be empty".to_string(),
                Some("Provide a valid database username".to_string())
            ));
        }

        if self.database.is_empty() {
            return Err(ServerError::validation_error(
                "Database name cannot be empty".to_string(),
                Some("Provide a valid database name".to_string())
            ));
        }

        if self.port == 0 {
            return Err(ServerError::validation_error(
                "Database port cannot be 0".to_string(),
                Some("Use a port number between 1 and 65535".to_string())
            ));
        }

        if self.connection_timeout == 0 {
            return Err(ServerError::validation_error(
                "Connection timeout cannot be 0".to_string(),
                Some("Set connection_timeout to a positive number of seconds".to_string())
            ));
        }

        if self.max_connections == 0 {
            return Err(ServerError::validation_error(
                "Max connections cannot be 0".to_string(),
                Some("Set max_connections to a positive number".to_string())
            ));
        }

        Ok(())
    }
}

impl EnvironmentConfig {
    /// Create a new environment configuration
    pub fn new(name: String, database: DatabaseConfig) -> Self {
        Self {
            name: name.clone(),
            description: None,
            database,
            connection_pool: PoolConfig::default(),
            enabled: true,
        }
    }

    /// Validate the environment configuration
    pub fn validate(&self) -> Result<()> {
        // Validate environment name
        if self.name.is_empty() {
            return Err(ServerError::validation_error(
                "Environment name cannot be empty".to_string(),
                Some("Provide a valid environment name (e.g., 'dev', 'sit', 'uat', 'prod')".to_string())
            ));
        }

        // Validate environment name format (alphanumeric, hyphens, underscores only)
        if !self.name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(ServerError::validation_error(
                format!("Invalid environment name '{}': only alphanumeric characters, hyphens, and underscores are allowed", self.name),
                Some("Use names like 'dev', 'sit-1', 'uat_env', etc.".to_string())
            ));
        }

        // Validate database configuration
        self.database.validate()?;

        // Validate connection pool configuration
        self.connection_pool.validate(&self.name)?;

        Ok(())
    }

    /// Get the connection URL for this environment
    pub fn connection_url(&self) -> String {
        self.database.build_connection_url()
    }

    /// Get the masked connection URL for logging
    pub fn masked_connection_url(&self) -> String {
        self.database.masked_connection_url()
    }
}

impl PoolConfig {
    /// Validate the pool configuration
    pub fn validate(&self, env_name: &str) -> Result<()> {
        if self.max_connections == 0 {
            return Err(ServerError::validation_error(
                format!("Environment '{}': max_connections cannot be 0", env_name),
                Some("Set max_connections to a positive number".to_string())
            ));
        }

        if self.min_connections > self.max_connections {
            return Err(ServerError::validation_error(
                format!("Environment '{}': min_connections ({}) cannot be greater than max_connections ({})", 
                    env_name, self.min_connections, self.max_connections),
                Some("Ensure min_connections <= max_connections".to_string())
            ));
        }

        if self.connection_timeout == 0 {
            return Err(ServerError::validation_error(
                format!("Environment '{}': connection_timeout cannot be 0", env_name),
                Some("Set connection_timeout to a positive number of seconds".to_string())
            ));
        }

        if self.idle_timeout == 0 {
            return Err(ServerError::validation_error(
                format!("Environment '{}': idle_timeout cannot be 0", env_name),
                Some("Set idle_timeout to a positive number of seconds".to_string())
            ));
        }

        Ok(())
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            connection_timeout: default_connection_timeout(),
            idle_timeout: default_idle_timeout(),
        }
    }
}

/// MCP protocol configuration section
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpConfig {
    /// MCP protocol version
    pub protocol_version: String,
    /// Server name
    pub server_name: String,
    /// Server version
    pub server_version: String,
}

/// Legacy connection configuration for backward compatibility
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// MySQL database connection URL
    pub database_url: String,
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        
        if !path.exists() {
            return Err(ServerError::validation_error(
                format!("Configuration file not found: {}", path.display()),
                Some("Create a config.toml file or specify a different path".to_string())
            ));
        }

        let content = fs::read_to_string(path)
            .map_err(|e| ServerError::validation_error(
                format!("Failed to read configuration file: {}", e),
                Some(format!("Check file permissions for: {}", path.display()))
            ))?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| ServerError::validation_error(
                format!("Failed to parse configuration file: {}", e),
                Some("Check TOML syntax in config.toml".to_string())
            ))?;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Load configuration with fallback to default file locations
    pub fn load() -> Result<Self> {
        // Try different config file locations in order of preference
        let config_paths = [
            "config.toml",
            "./config.toml",
            "config/config.toml",
        ];

        for path in &config_paths {
            if Path::new(path).exists() {
                return Self::from_file(path);
            }
        }

        // If no config file found, try environment variables as fallback
        Self::from_env()
    }

    /// Load configuration from environment variables (fallback)
    pub fn from_env() -> Result<Self> {
        use std::env;

        // Try to get individual database components first
        let db_host = env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
        let db_port_str = env::var("DB_PORT").unwrap_or_else(|_| "3306".to_string());
        let db_port: u16 = db_port_str.parse()
            .map_err(|_| ServerError::validation_error(
                format!("Invalid DB_PORT value: {}", db_port_str),
                Some("DB_PORT must be a valid number between 1 and 65535".to_string())
            ))?;
        
        let db_username = env::var("DB_USERNAME").or_else(|_| env::var("DB_USER"));
        let db_password = env::var("DB_PASSWORD").or_else(|_| env::var("DB_PASS"));
        let db_database = env::var("DB_DATABASE").or_else(|_| env::var("DB_NAME"));

        // If individual components are not available, try DATABASE_URL as fallback
        let database_config = if let (Ok(username), Ok(password), Ok(database)) = 
            (db_username, db_password, db_database) {
            DatabaseConfig {
                host: db_host,
                port: db_port,
                username,
                password,
                database,
                connection_timeout: default_connection_timeout(),
                max_connections: default_max_connections(),
            }
        } else if let Ok(database_url) = env::var("DATABASE_URL") {
            // Parse DATABASE_URL as fallback
            Self::parse_database_url(&database_url)?
        } else {
            return Err(ServerError::validation_error(
                "No configuration file found and database environment variables are not set".to_string(),
                Some("Create a config.toml file or set DB_HOST, DB_USERNAME, DB_PASSWORD, DB_DATABASE environment variables".to_string())
            ));
        };

        let port_str = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
        let port: u16 = port_str.parse()
            .map_err(|_| ServerError::validation_error(
                format!("Invalid PORT value: {}", port_str),
                Some("PORT must be a valid number between 1 and 65535".to_string())
            ))?;

        let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

        Ok(Config {
            server: ServerConfig {
                port,
                log_level,
            },
            database: Some(database_config),
            environments: None,
            default_environment: None,
            mcp: McpConfig {
                protocol_version: "2024-11-05".to_string(),
                server_name: "mysql-mcp-server".to_string(),
                server_version: "0.1.0".to_string(),
            },
        })
    }

    /// Parse DATABASE_URL into DatabaseConfig components
    fn parse_database_url(url: &str) -> Result<DatabaseConfig> {
        if !url.starts_with("mysql://") {
            return Err(ServerError::validation_error(
                "DATABASE_URL must start with 'mysql://'".to_string(),
                Some(url.to_string())
            ));
        }

        // Remove mysql:// prefix
        let url_without_scheme = &url[8..];
        
        // Split by @ to separate credentials from host/database
        let parts: Vec<&str> = url_without_scheme.split('@').collect();
        if parts.len() != 2 {
            return Err(ServerError::validation_error(
                "Invalid DATABASE_URL format".to_string(),
                Some("Expected format: mysql://username:password@host:port/database".to_string())
            ));
        }

        // Parse credentials
        let credentials = parts[0];
        let cred_parts: Vec<&str> = credentials.split(':').collect();
        if cred_parts.len() != 2 {
            return Err(ServerError::validation_error(
                "Invalid credentials in DATABASE_URL".to_string(),
                Some("Expected format: username:password".to_string())
            ));
        }
        let username = cred_parts[0].to_string();
        let password = cred_parts[1].to_string();

        // Parse host/port/database
        let host_db = parts[1];
        let host_db_parts: Vec<&str> = host_db.split('/').collect();
        if host_db_parts.len() != 2 {
            return Err(ServerError::validation_error(
                "Invalid host/database in DATABASE_URL".to_string(),
                Some("Expected format: host:port/database".to_string())
            ));
        }

        let host_port = host_db_parts[0];
        let database = host_db_parts[1].to_string();

        // Parse host and port
        let (host, port) = if host_port.contains(':') {
            let hp_parts: Vec<&str> = host_port.split(':').collect();
            if hp_parts.len() != 2 {
                return Err(ServerError::validation_error(
                    "Invalid host:port in DATABASE_URL".to_string(),
                    None
                ));
            }
            let port: u16 = hp_parts[1].parse()
                .map_err(|_| ServerError::validation_error(
                    format!("Invalid port number: {}", hp_parts[1]),
                    None
                ))?;
            (hp_parts[0].to_string(), port)
        } else {
            (host_port.to_string(), default_mysql_port())
        };

        Ok(DatabaseConfig {
            host,
            port,
            username,
            password,
            database,
            connection_timeout: default_connection_timeout(),
            max_connections: default_max_connections(),
        })
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate server configuration
        if self.server.port == 0 {
            return Err(ServerError::validation_error(
                "Server port cannot be 0".to_string(),
                Some("Use a port number between 1 and 65535".to_string())
            ));
        }

        // Validate log level
        match self.server.log_level.to_lowercase().as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {},
            _ => {
                return Err(ServerError::validation_error(
                    format!("Invalid log level: {}", self.server.log_level),
                    Some("Valid log levels: trace, debug, info, warn, error".to_string())
                ));
            }
        }

        // Validate database configuration (legacy or multi-environment)
        match (&self.database, &self.environments) {
            (Some(db_config), None) => {
                // Legacy single database configuration
                db_config.validate()?;
            },
            (None, Some(environments)) => {
                // Multi-environment configuration
                if environments.is_empty() {
                    return Err(ServerError::validation_error(
                        "No environments configured".to_string(),
                        Some("Configure at least one environment in the [environments] section".to_string())
                    ));
                }

                // Validate each environment
                for (env_name, env_config) in environments {
                    if env_name != &env_config.name {
                        return Err(ServerError::validation_error(
                            format!("Environment key '{}' does not match environment name '{}'", env_name, env_config.name),
                            Some("Ensure environment keys match their name fields".to_string())
                        ));
                    }
                    env_config.validate()?;
                }

                // Validate default environment if specified
                if let Some(default_env) = &self.default_environment {
                    if !environments.contains_key(default_env) {
                        return Err(ServerError::validation_error(
                            format!("Default environment '{}' is not configured", default_env),
                            Some(format!("Available environments: {}", environments.keys().cloned().collect::<Vec<_>>().join(", ")))
                        ));
                    }
                }

                // Check that at least one environment is enabled
                let enabled_count = environments.values().filter(|env| env.enabled).count();
                if enabled_count == 0 {
                    return Err(ServerError::validation_error(
                        "No environments are enabled".to_string(),
                        Some("Enable at least one environment by setting enabled = true".to_string())
                    ));
                }
            },
            (Some(_), Some(_)) => {
                return Err(ServerError::validation_error(
                    "Cannot specify both legacy 'database' and new 'environments' configuration".to_string(),
                    Some("Use either the legacy [database] section or the new [environments] section, not both".to_string())
                ));
            },
            (None, None) => {
                return Err(ServerError::validation_error(
                    "No database configuration found".to_string(),
                    Some("Configure either a [database] section (legacy) or [environments] section (multi-environment)".to_string())
                ));
            }
        }

        Ok(())
    }

    /// Convert to legacy ConnectionConfig for backward compatibility
    pub fn to_connection_config(&self) -> ConnectionConfig {
        match &self.database {
            Some(db_config) => ConnectionConfig {
                database_url: db_config.build_connection_url(),
            },
            None => {
                // If using multi-environment, try to use default environment or first available
                if let Some(environments) = &self.environments {
                    let env_config = if let Some(default_env) = &self.default_environment {
                        environments.get(default_env)
                    } else {
                        environments.values().find(|env| env.enabled)
                    };
                    
                    if let Some(env) = env_config {
                        ConnectionConfig {
                            database_url: env.database.build_connection_url(),
                        }
                    } else {
                        panic!("No valid database configuration found for legacy compatibility")
                    }
                } else {
                    panic!("No database configuration found for legacy compatibility")
                }
            }
        }
    }

    /// Check if this is a multi-environment configuration
    pub fn is_multi_environment(&self) -> bool {
        self.environments.is_some()
    }

    /// Get all configured environments
    pub fn get_environments(&self) -> Option<&HashMap<String, EnvironmentConfig>> {
        self.environments.as_ref()
    }

    /// Get a specific environment configuration
    pub fn get_environment(&self, name: &str) -> Option<&EnvironmentConfig> {
        self.environments.as_ref()?.get(name)
    }

    /// Get the default environment name
    pub fn get_default_environment(&self) -> Option<&str> {
        self.default_environment.as_deref()
    }

    /// Get all enabled environment names
    pub fn get_enabled_environments(&self) -> Vec<String> {
        match &self.environments {
            Some(environments) => {
                environments.values()
                    .filter(|env| env.enabled)
                    .map(|env| env.name.clone())
                    .collect()
            },
            None => {
                // Legacy single database mode
                vec!["default".to_string()]
            }
        }
    }

    /// Get the legacy database configuration (for backward compatibility)
    pub fn get_legacy_database(&self) -> Option<&DatabaseConfig> {
        self.database.as_ref()
    }
}

// Default value functions for serde
fn default_mysql_port() -> u16 {
    3306
}

fn default_connection_timeout() -> u64 {
    30
}

fn default_max_connections() -> u32 {
    10
}

fn default_min_connections() -> u32 {
    1
}

fn default_idle_timeout() -> u64 {
    600 // 10 minutes
}

fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_config_build_connection_url() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 3306,
            username: "testuser".to_string(),
            password: "testpass".to_string(),
            database: "testdb".to_string(),
            connection_timeout: 30,
            max_connections: 10,
        };

        let url = config.build_connection_url();
        assert_eq!(url, "mysql://testuser:testpass@localhost:3306/testdb");
    }

    #[test]
    fn test_database_config_masked_connection_url() {
        let config = DatabaseConfig {
            host: "localhost".to_string(),
            port: 3306,
            username: "testuser".to_string(),
            password: "secretpassword".to_string(),
            database: "testdb".to_string(),
            connection_timeout: 30,
            max_connections: 10,
        };

        let masked_url = config.masked_connection_url();
        assert_eq!(masked_url, "mysql://testuser:****@localhost:3306/testdb");
        assert!(!masked_url.contains("secretpassword"));
    }

    #[test]
    fn test_parse_database_url() {
        let url = "mysql://user:pass@example.com:3307/mydb";
        let config = Config::parse_database_url(url).unwrap();

        assert_eq!(config.host, "example.com");
        assert_eq!(config.port, 3307);
        assert_eq!(config.username, "user");
        assert_eq!(config.password, "pass");
        assert_eq!(config.database, "mydb");
    }

    #[test]
    fn test_parse_database_url_default_port() {
        let url = "mysql://user:pass@localhost/mydb";
        let config = Config::parse_database_url(url).unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 3306); // Default port
        assert_eq!(config.username, "user");
        assert_eq!(config.password, "pass");
        assert_eq!(config.database, "mydb");
    }

    #[test]
    fn test_parse_invalid_database_url() {
        let invalid_urls = vec![
            "http://user:pass@localhost/db",  // Wrong scheme
            "mysql://user@localhost/db",      // Missing password
            "mysql://user:pass@localhost",    // Missing database
            "mysql://localhost/db",           // Missing credentials
        ];

        for url in invalid_urls {
            assert!(Config::parse_database_url(url).is_err());
        }
    }

    #[test]
    fn test_config_from_file() {
        // Create a temporary config file
        let config_content = r#"
[server]
port = 9999
log_level = "debug"

[database]
host = "testhost"
port = 3307
username = "testuser"
password = "testpass"
database = "testdb"
connection_timeout = 60
max_connections = 20

[mcp]
protocol_version = "2024-11-05"
server_name = "test-server"
server_version = "0.1.0"
"#;

        // Write to a temporary file
        use std::io::Write;
        let mut temp_file = std::fs::File::create("test_temp_config.toml").unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();
        drop(temp_file);

        // Load and validate config
        let config = Config::from_file("test_temp_config.toml").unwrap();
        
        assert_eq!(config.server.port, 9999);
        assert_eq!(config.server.log_level, "debug");
        assert!(config.database.is_some());
        let db_config = config.database.as_ref().unwrap();
        assert_eq!(db_config.host, "testhost");
        assert_eq!(db_config.port, 3307);
        assert_eq!(db_config.username, "testuser");
        assert_eq!(db_config.password, "testpass");
        assert_eq!(db_config.database, "testdb");
        assert_eq!(db_config.connection_timeout, 60);
        assert_eq!(db_config.max_connections, 20);

        // Test connection URL building
        let connection_url = db_config.build_connection_url();
        assert_eq!(connection_url, "mysql://testuser:testpass@testhost:3307/testdb");

        // Test masked URL
        let masked_url = db_config.masked_connection_url();
        assert_eq!(masked_url, "mysql://testuser:****@testhost:3307/testdb");

        // Clean up
        std::fs::remove_file("test_temp_config.toml").ok();
    }

    #[test]
    fn test_current_config_file() {
        // Test loading the actual config.toml file if it exists
        if std::path::Path::new("config.toml").exists() {
            let config = Config::from_file("config.toml").unwrap();
            
            // Validate that the config is properly structured
            if let Some(db_config) = &config.database {
                assert!(!db_config.host.is_empty());
                assert!(!db_config.username.is_empty());
                assert!(!db_config.database.is_empty());
                assert!(db_config.port > 0);
                assert!(config.server.port > 0);
                
                // Test that connection URL can be built
                let connection_url = db_config.build_connection_url();
                assert!(connection_url.starts_with("mysql://"));
                
                // Test that masked URL doesn't contain password
                let masked_url = db_config.masked_connection_url();
                assert!(masked_url.contains("****"));
                assert!(!masked_url.contains(&db_config.password));
                
                println!("Current config validation passed!");
                println!("Host: {}", db_config.host);
                println!("Username: {}", db_config.username);
                println!("Database: {}", db_config.database);
                println!("Masked URL: {}", masked_url);
            }
        }
    }

    #[test]
    fn test_multi_environment_config() {
        // Create a multi-environment config file
        let config_content = r#"
default_environment = "dev"

[server]
port = 8080
log_level = "info"

[environments.dev]
name = "dev"
description = "Development environment"
enabled = true

[environments.dev.database]
host = "dev-db.example.com"
port = 3306
username = "dev_user"
password = "dev_pass"
database = "dev_db"
connection_timeout = 30

[environments.dev.connection_pool]
max_connections = 5
min_connections = 1
connection_timeout = 30
idle_timeout = 300

[environments.uat]
name = "uat"
description = "User Acceptance Testing environment"
enabled = true

[environments.uat.database]
host = "uat-db.example.com"
port = 3306
username = "uat_user"
password = "uat_pass"
database = "uat_db"
connection_timeout = 30

[environments.uat.connection_pool]
max_connections = 10
min_connections = 2
connection_timeout = 30
idle_timeout = 600

[mcp]
protocol_version = "2024-11-05"
server_name = "mysql-mcp-server"
server_version = "0.1.0"
"#;

        // Write to a temporary file
        use std::io::Write;
        let mut temp_file = std::fs::File::create("test_multi_env_config.toml").unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();
        drop(temp_file);

        // Load and validate config
        let config = Config::from_file("test_multi_env_config.toml").unwrap();
        
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.log_level, "info");
        assert!(config.is_multi_environment());
        assert_eq!(config.get_default_environment(), Some("dev"));
        
        // Test environments
        let environments = config.get_environments().unwrap();
        assert_eq!(environments.len(), 2);
        
        // Test dev environment
        let dev_env = config.get_environment("dev").unwrap();
        assert_eq!(dev_env.name, "dev");
        assert_eq!(dev_env.description, Some("Development environment".to_string()));
        assert!(dev_env.enabled);
        assert_eq!(dev_env.database.host, "dev-db.example.com");
        assert_eq!(dev_env.database.username, "dev_user");
        assert_eq!(dev_env.connection_pool.max_connections, 5);
        assert_eq!(dev_env.connection_pool.min_connections, 1);
        
        // Test UAT environment
        let uat_env = config.get_environment("uat").unwrap();
        assert_eq!(uat_env.name, "uat");
        assert_eq!(uat_env.description, Some("User Acceptance Testing environment".to_string()));
        assert!(uat_env.enabled);
        assert_eq!(uat_env.database.host, "uat-db.example.com");
        assert_eq!(uat_env.database.username, "uat_user");
        assert_eq!(uat_env.connection_pool.max_connections, 10);
        assert_eq!(uat_env.connection_pool.min_connections, 2);
        
        // Test enabled environments
        let enabled_envs = config.get_enabled_environments();
        assert_eq!(enabled_envs.len(), 2);
        assert!(enabled_envs.contains(&"dev".to_string()));
        assert!(enabled_envs.contains(&"uat".to_string()));
        
        // Test connection URLs
        let dev_url = dev_env.connection_url();
        assert_eq!(dev_url, "mysql://dev_user:dev_pass@dev-db.example.com:3306/dev_db");
        
        let dev_masked_url = dev_env.masked_connection_url();
        assert_eq!(dev_masked_url, "mysql://dev_user:****@dev-db.example.com:3306/dev_db");
        assert!(!dev_masked_url.contains("dev_pass"));

        // Clean up
        std::fs::remove_file("test_multi_env_config.toml").ok();
    }

    #[test]
    fn test_environment_config_validation() {
        // Test invalid environment name
        let mut env_config = EnvironmentConfig::new(
            "".to_string(),
            DatabaseConfig {
                host: "localhost".to_string(),
                port: 3306,
                username: "user".to_string(),
                password: "pass".to_string(),
                database: "db".to_string(),
                connection_timeout: 30,
                max_connections: 10,
            }
        );
        assert!(env_config.validate().is_err());

        // Test invalid environment name with special characters
        env_config.name = "dev@env".to_string();
        assert!(env_config.validate().is_err());

        // Test valid environment name
        env_config.name = "dev-env_1".to_string();
        assert!(env_config.validate().is_ok());
    }

    #[test]
    fn test_pool_config_validation() {
        let pool_config = PoolConfig {
            max_connections: 0,
            min_connections: 1,
            connection_timeout: 30,
            idle_timeout: 600,
        };
        assert!(pool_config.validate("test").is_err());

        let pool_config = PoolConfig {
            max_connections: 5,
            min_connections: 10, // min > max
            connection_timeout: 30,
            idle_timeout: 600,
        };
        assert!(pool_config.validate("test").is_err());

        let pool_config = PoolConfig {
            max_connections: 10,
            min_connections: 5,
            connection_timeout: 0, // invalid timeout
            idle_timeout: 600,
        };
        assert!(pool_config.validate("test").is_err());

        let pool_config = PoolConfig {
            max_connections: 10,
            min_connections: 5,
            connection_timeout: 30,
            idle_timeout: 600,
        };
        assert!(pool_config.validate("test").is_ok());
    }

    #[test]
    fn test_config_validation_errors() {
        // Test config with both legacy and multi-environment
        let config_content = r#"
[server]
port = 8080
log_level = "info"

[database]
host = "localhost"
port = 3306
username = "user"
password = "pass"
database = "db"

[environments.dev]
name = "dev"
enabled = true

[environments.dev.database]
host = "dev-db.example.com"
port = 3306
username = "dev_user"
password = "dev_pass"
database = "dev_db"

[mcp]
protocol_version = "2024-11-05"
server_name = "mysql-mcp-server"
server_version = "0.1.0"
"#;

        use std::io::Write;
        let mut temp_file = std::fs::File::create("test_invalid_config.toml").unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();
        drop(temp_file);

        // This should fail validation
        let result = Config::from_file("test_invalid_config.toml");
        assert!(result.is_err());

        // Clean up
        std::fs::remove_file("test_invalid_config.toml").ok();
    }

    #[test]
    fn test_example_multi_env_config_validation() {
        // Test the actual example multi-environment config file
        if std::path::Path::new("config.multi-env.example.toml").exists() {
            let config = Config::from_file("config.multi-env.example.toml").unwrap();
            
            // Validate it's multi-environment
            assert!(config.is_multi_environment());
            
            // Check environments
            let environments = config.get_environments().unwrap();
            assert!(environments.len() >= 3); // Should have dev, sit, uat, prod
            
            // Check specific environments exist
            assert!(config.get_environment("dev").is_some());
            assert!(config.get_environment("sit").is_some());
            assert!(config.get_environment("uat").is_some());
            assert!(config.get_environment("prod").is_some());
            
            // Check default environment
            assert_eq!(config.get_default_environment(), Some("dev"));
            assert!(config.get_environment("dev").is_some());
            
            // Check enabled environments (prod should be disabled by default)
            let enabled_envs = config.get_enabled_environments();
            assert!(enabled_envs.contains(&"dev".to_string()));
            assert!(enabled_envs.contains(&"sit".to_string()));
            assert!(enabled_envs.contains(&"uat".to_string()));
            assert!(!enabled_envs.contains(&"prod".to_string())); // Should be disabled
            
            // Validate each environment configuration
            for (name, env) in environments {
                assert_eq!(name, &env.name);
                assert!(env.validate().is_ok());
                
                // Check connection URLs can be built
                let url = env.connection_url();
                assert!(url.starts_with("mysql://"));
                
                let masked_url = env.masked_connection_url();
                assert!(masked_url.contains("****"));
                assert!(!masked_url.contains(&env.database.password));
            }
            
            println!("✅ Example multi-environment config validation passed!");
        } else {
            panic!("config.multi-env.example.toml file not found");
        }
    }
}