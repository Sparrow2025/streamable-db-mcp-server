//! Configuration structures for the MySQL MCP server

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use crate::{Result, ServerError};

/// Complete server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
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
            database: database_config,
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
        // Validate database configuration
        if self.database.host.is_empty() {
            return Err(ServerError::validation_error(
                "Database host cannot be empty".to_string(),
                None
            ));
        }

        if self.database.username.is_empty() {
            return Err(ServerError::validation_error(
                "Database username cannot be empty".to_string(),
                None
            ));
        }

        if self.database.database.is_empty() {
            return Err(ServerError::validation_error(
                "Database name cannot be empty".to_string(),
                None
            ));
        }

        // Validate database port
        if self.database.port == 0 {
            return Err(ServerError::validation_error(
                "Database port cannot be 0".to_string(),
                Some("Use a port number between 1 and 65535".to_string())
            ));
        }

        // Validate server port
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

        // Validate connection settings
        if self.database.connection_timeout == 0 {
            return Err(ServerError::validation_error(
                "Connection timeout cannot be 0".to_string(),
                None
            ));
        }

        if self.database.max_connections == 0 {
            return Err(ServerError::validation_error(
                "Max connections cannot be 0".to_string(),
                None
            ));
        }

        Ok(())
    }

    /// Convert to legacy ConnectionConfig for backward compatibility
    pub fn to_connection_config(&self) -> ConnectionConfig {
        ConnectionConfig {
            database_url: self.database.build_connection_url(),
        }
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
        assert_eq!(config.database.host, "testhost");
        assert_eq!(config.database.port, 3307);
        assert_eq!(config.database.username, "testuser");
        assert_eq!(config.database.password, "testpass");
        assert_eq!(config.database.database, "testdb");
        assert_eq!(config.database.connection_timeout, 60);
        assert_eq!(config.database.max_connections, 20);

        // Test connection URL building
        let connection_url = config.database.build_connection_url();
        assert_eq!(connection_url, "mysql://testuser:testpass@testhost:3307/testdb");

        // Test masked URL
        let masked_url = config.database.masked_connection_url();
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
            assert!(!config.database.host.is_empty());
            assert!(!config.database.username.is_empty());
            assert!(!config.database.database.is_empty());
            assert!(config.database.port > 0);
            assert!(config.server.port > 0);
            
            // Test that connection URL can be built
            let connection_url = config.database.build_connection_url();
            assert!(connection_url.starts_with("mysql://"));
            
            // Test that masked URL doesn't contain password
            let masked_url = config.database.masked_connection_url();
            assert!(masked_url.contains("****"));
            assert!(!masked_url.contains(&config.database.password));
            
            println!("Current config validation passed!");
            println!("Host: {}", config.database.host);
            println!("Username: {}", config.database.username);
            println!("Database: {}", config.database.database);
            println!("Masked URL: {}", masked_url);
        }
    }
}