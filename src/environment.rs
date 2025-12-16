//! Environment Manager for multi-database connection support
//! 
//! This module provides the EnvironmentManager component that handles:
//! - Loading and validating environment configurations
//! - Managing environment metadata and connection parameters
//! - Providing environment discovery and listing capabilities
//! - Ensuring credential isolation and secure storage

use crate::{Result, ServerError};
use crate::config::{Config, EnvironmentConfig};
use std::collections::HashMap;
use tracing::{info, warn, debug};

/// Environment status information
#[derive(Debug, Clone, PartialEq)]
pub enum EnvironmentStatus {
    /// Environment is enabled and ready for connections
    Enabled,
    /// Environment is disabled in configuration
    Disabled,
    /// Environment configuration is invalid
    Invalid(String),
}

/// Environment metadata containing configuration and status
#[derive(Debug, Clone)]
pub struct EnvironmentMetadata {
    /// Environment configuration
    pub config: EnvironmentConfig,
    /// Current status of the environment
    pub status: EnvironmentStatus,
    /// Whether this environment is the default
    pub is_default: bool,
}

/// Environment Manager responsible for managing multiple database environments
#[derive(Debug)]
pub struct EnvironmentManager {
    /// Map of environment name to environment metadata
    environments: HashMap<String, EnvironmentMetadata>,
    /// Default environment name (if configured)
    default_environment: Option<String>,
    /// Whether this is a legacy single-database configuration
    is_legacy_mode: bool,
}

impl EnvironmentManager {
    /// Create a new EnvironmentManager from configuration
    pub fn load_from_config(config: &Config) -> Result<Self> {
        info!("Loading environment configuration");
        
        let mut manager = if config.is_multi_environment() {
            // Multi-environment configuration
            Self::load_multi_environment_config(config)?
        } else {
            // Legacy single database configuration
            Self::load_legacy_config(config)?
        };

        // Validate all environments
        manager.validate_all_environments()?;
        
        info!("Environment manager loaded successfully with {} environments", 
              manager.environments.len());
        
        Ok(manager)
    }

    /// Load multi-environment configuration
    fn load_multi_environment_config(config: &Config) -> Result<Self> {
        let environments_config = config.get_environments()
            .ok_or_else(|| ServerError::configuration_error(
                "environments".to_string(),
                "No environments configuration found".to_string()
            ))?;

        let default_environment = config.get_default_environment().map(|s| s.to_string());
        let mut environments = HashMap::new();

        for (env_name, env_config) in environments_config {
            let is_default = default_environment.as_ref() == Some(env_name);
            
            let status = if env_config.enabled {
                EnvironmentStatus::Enabled
            } else {
                EnvironmentStatus::Disabled
            };

            let metadata = EnvironmentMetadata {
                config: env_config.clone(),
                status,
                is_default,
            };

            environments.insert(env_name.clone(), metadata);
            debug!("Loaded environment '{}' (enabled: {}, default: {})", 
                   env_name, env_config.enabled, is_default);
        }

        Ok(Self {
            environments,
            default_environment,
            is_legacy_mode: false,
        })
    }

    /// Load legacy single database configuration
    fn load_legacy_config(config: &Config) -> Result<Self> {
        let db_config = config.get_legacy_database()
            .ok_or_else(|| ServerError::configuration_error(
                "database".to_string(),
                "No database configuration found".to_string()
            ))?;

        // Create a single "default" environment from legacy config
        let env_config = EnvironmentConfig::new("default".to_string(), db_config.clone());
        
        let metadata = EnvironmentMetadata {
            config: env_config,
            status: EnvironmentStatus::Enabled,
            is_default: true,
        };

        let mut environments = HashMap::new();
        environments.insert("default".to_string(), metadata);

        Ok(Self {
            environments,
            default_environment: Some("default".to_string()),
            is_legacy_mode: true,
        })
    }

    /// Validate all environment configurations
    fn validate_all_environments(&mut self) -> Result<()> {
        let mut validation_errors = Vec::new();

        for (env_name, metadata) in &mut self.environments {
            match metadata.config.validate() {
                Ok(()) => {
                    // Environment is valid, ensure status reflects this
                    if metadata.status == EnvironmentStatus::Disabled {
                        // Keep disabled status
                    } else {
                        metadata.status = EnvironmentStatus::Enabled;
                    }
                }
                Err(err) => {
                    // Environment is invalid
                    let error_msg = err.user_message();
                    warn!("Environment '{}' validation failed: {}", env_name, error_msg);
                    metadata.status = EnvironmentStatus::Invalid(error_msg.clone());
                    validation_errors.push(format!("Environment '{}': {}", env_name, error_msg));
                }
            }
        }

        // If we have validation errors, return them as a configuration error
        if !validation_errors.is_empty() {
            return Err(ServerError::configuration_error(
                "environments".to_string(),
                format!("Environment validation failed: {}", validation_errors.join("; "))
            ));
        }

        // Validate default environment exists and is enabled
        if let Some(default_env) = &self.default_environment {
            match self.environments.get(default_env) {
                Some(metadata) => {
                    match metadata.status {
                        EnvironmentStatus::Enabled => {
                            debug!("Default environment '{}' is valid and enabled", default_env);
                        }
                        EnvironmentStatus::Disabled => {
                            return Err(ServerError::configuration_error(
                                "default_environment".to_string(),
                                format!("Default environment '{}' is disabled", default_env)
                            ));
                        }
                        EnvironmentStatus::Invalid(ref error) => {
                            return Err(ServerError::configuration_error(
                                "default_environment".to_string(),
                                format!("Default environment '{}' is invalid: {}", default_env, error)
                            ));
                        }
                    }
                }
                None => {
                    return Err(ServerError::configuration_error(
                        "default_environment".to_string(),
                        format!("Default environment '{}' does not exist", default_env)
                    ));
                }
            }
        }

        // Ensure at least one environment is enabled
        let enabled_count = self.environments.values()
            .filter(|metadata| metadata.status == EnvironmentStatus::Enabled)
            .count();

        if enabled_count == 0 {
            return Err(ServerError::configuration_error(
                "environments".to_string(),
                "No environments are enabled".to_string()
            ));
        }

        Ok(())
    }

    /// Get environment configuration by name
    pub fn get_environment(&self, name: &str) -> Option<&EnvironmentConfig> {
        self.environments.get(name).map(|metadata| &metadata.config)
    }

    /// Get environment metadata by name
    pub fn get_environment_metadata(&self, name: &str) -> Option<&EnvironmentMetadata> {
        self.environments.get(name)
    }

    /// List all environment names
    pub fn list_environments(&self) -> Vec<&str> {
        self.environments.keys().map(|s| s.as_str()).collect()
    }

    /// List all enabled environment names
    pub fn list_enabled_environments(&self) -> Vec<&str> {
        self.environments.iter()
            .filter(|(_, metadata)| metadata.status == EnvironmentStatus::Enabled)
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Get environment status information for all environments
    pub fn get_environment_status_report(&self) -> HashMap<String, EnvironmentStatusReport> {
        self.environments.iter()
            .map(|(name, metadata)| {
                let report = EnvironmentStatusReport {
                    name: name.clone(),
                    description: metadata.config.description.clone(),
                    status: metadata.status.clone(),
                    is_default: metadata.is_default,
                    is_legacy: self.is_legacy_mode,
                    connection_info: ConnectionInfo {
                        host: metadata.config.database.host.clone(),
                        port: metadata.config.database.port,
                        database: metadata.config.database.database.clone(),
                        // Never expose password in status reports
                        password_configured: !metadata.config.database.password.is_empty(),
                    },
                    pool_config: PoolConfigInfo {
                        max_connections: metadata.config.connection_pool.max_connections,
                        min_connections: metadata.config.connection_pool.min_connections,
                        connection_timeout: metadata.config.connection_pool.connection_timeout,
                        idle_timeout: metadata.config.connection_pool.idle_timeout,
                    },
                };
                (name.clone(), report)
            })
            .collect()
    }

    /// Validate a specific environment by name
    pub fn validate_environment(&self, name: &str) -> Result<()> {
        let metadata = self.environments.get(name)
            .ok_or_else(|| ServerError::validation_error(
                format!("Environment '{}' not found", name),
                Some(name.to_string())
            ))?;

        match &metadata.status {
            EnvironmentStatus::Enabled => Ok(()),
            EnvironmentStatus::Disabled => Err(ServerError::validation_error(
                format!("Environment '{}' is disabled", name),
                Some(name.to_string())
            )),
            EnvironmentStatus::Invalid(error) => Err(ServerError::validation_error(
                format!("Environment '{}' is invalid: {}", name, error),
                Some(name.to_string())
            )),
        }
    }

    /// Get the default environment name
    pub fn get_default_environment(&self) -> Option<&str> {
        self.default_environment.as_deref()
    }

    /// Check if this is legacy mode (single database)
    pub fn is_legacy_mode(&self) -> bool {
        self.is_legacy_mode
    }

    /// Get the number of configured environments
    pub fn environment_count(&self) -> usize {
        self.environments.len()
    }

    /// Get the number of enabled environments
    pub fn enabled_environment_count(&self) -> usize {
        self.environments.values()
            .filter(|metadata| metadata.status == EnvironmentStatus::Enabled)
            .count()
    }

    /// Check if an environment exists
    pub fn has_environment(&self, name: &str) -> bool {
        self.environments.contains_key(name)
    }

    /// Get connection URL for an environment (masked for logging)
    pub fn get_masked_connection_url(&self, name: &str) -> Option<String> {
        self.get_environment(name).map(|env| env.masked_connection_url())
    }

    /// Get connection URL for an environment (with credentials - use carefully!)
    pub fn get_connection_url(&self, name: &str) -> Option<String> {
        self.get_environment(name).map(|env| env.connection_url())
    }
}

/// Environment status report for external consumption
#[derive(Debug, Clone)]
pub struct EnvironmentStatusReport {
    /// Environment name
    pub name: String,
    /// Environment description
    pub description: Option<String>,
    /// Current status
    pub status: EnvironmentStatus,
    /// Whether this is the default environment
    pub is_default: bool,
    /// Whether this is from legacy configuration
    pub is_legacy: bool,
    /// Connection information (credentials masked)
    pub connection_info: ConnectionInfo,
    /// Connection pool configuration
    pub pool_config: PoolConfigInfo,
}

/// Connection information for status reports (credentials masked)
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Database host
    pub host: String,
    /// Database port
    pub port: u16,
    /// Database name
    pub database: String,
    /// Whether password is configured (never expose actual password)
    pub password_configured: bool,
}

/// Connection pool configuration information
#[derive(Debug, Clone)]
pub struct PoolConfigInfo {
    /// Maximum connections in pool
    pub max_connections: u32,
    /// Minimum connections in pool
    pub min_connections: u32,
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    /// Idle timeout in seconds
    pub idle_timeout: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DatabaseConfig, PoolConfig, ServerConfig, McpConfig};

    fn create_test_database_config(name: &str) -> DatabaseConfig {
        DatabaseConfig {
            host: format!("{}-db.example.com", name),
            port: 3306,
            username: format!("{}_user", name),
            password: format!("{}_password", name),
            database: format!("{}_db", name),
            connection_timeout: 30,
            max_connections: 10,
        }
    }

    fn create_test_environment_config(name: &str) -> EnvironmentConfig {
        EnvironmentConfig {
            name: name.to_string(),
            description: Some(format!("{} environment", name)),
            database: create_test_database_config(name),
            connection_pool: PoolConfig::default(),
            enabled: true,
        }
    }

    fn create_test_config_multi_env() -> Config {
        let mut environments = HashMap::new();
        environments.insert("dev".to_string(), create_test_environment_config("dev"));
        environments.insert("uat".to_string(), create_test_environment_config("uat"));
        
        Config {
            server: ServerConfig {
                port: 8080,
                log_level: "info".to_string(),
            },
            database: None,
            environments: Some(environments),
            default_environment: Some("dev".to_string()),
            mcp: McpConfig {
                protocol_version: "2024-11-05".to_string(),
                server_name: "test-server".to_string(),
                server_version: "0.1.0".to_string(),
            },
        }
    }

    fn create_test_config_legacy() -> Config {
        Config {
            server: ServerConfig {
                port: 8080,
                log_level: "info".to_string(),
            },
            database: Some(create_test_database_config("default")),
            environments: None,
            default_environment: None,
            mcp: McpConfig {
                protocol_version: "2024-11-05".to_string(),
                server_name: "test-server".to_string(),
                server_version: "0.1.0".to_string(),
            },
        }
    }

    #[test]
    fn test_load_multi_environment_config() {
        let config = create_test_config_multi_env();
        let manager = EnvironmentManager::load_from_config(&config).unwrap();

        assert!(!manager.is_legacy_mode());
        assert_eq!(manager.environment_count(), 2);
        assert_eq!(manager.enabled_environment_count(), 2);
        assert_eq!(manager.get_default_environment(), Some("dev"));

        // Test environment listing
        let env_names = manager.list_environments();
        assert!(env_names.contains(&"dev"));
        assert!(env_names.contains(&"uat"));

        let enabled_envs = manager.list_enabled_environments();
        assert_eq!(enabled_envs.len(), 2);
    }

    #[test]
    fn test_load_legacy_config() {
        let config = create_test_config_legacy();
        let manager = EnvironmentManager::load_from_config(&config).unwrap();

        assert!(manager.is_legacy_mode());
        assert_eq!(manager.environment_count(), 1);
        assert_eq!(manager.enabled_environment_count(), 1);
        assert_eq!(manager.get_default_environment(), Some("default"));

        // Test environment access
        let default_env = manager.get_environment("default").unwrap();
        assert_eq!(default_env.name, "default");
        assert_eq!(default_env.database.host, "default-db.example.com");
    }

    #[test]
    fn test_environment_validation() {
        let config = create_test_config_multi_env();
        let manager = EnvironmentManager::load_from_config(&config).unwrap();

        // Test valid environment
        assert!(manager.validate_environment("dev").is_ok());
        assert!(manager.validate_environment("uat").is_ok());

        // Test non-existent environment
        assert!(manager.validate_environment("nonexistent").is_err());
    }

    #[test]
    fn test_environment_status_report() {
        let config = create_test_config_multi_env();
        let manager = EnvironmentManager::load_from_config(&config).unwrap();

        let status_report = manager.get_environment_status_report();
        assert_eq!(status_report.len(), 2);

        let dev_report = status_report.get("dev").unwrap();
        assert_eq!(dev_report.name, "dev");
        assert_eq!(dev_report.status, EnvironmentStatus::Enabled);
        assert!(dev_report.is_default);
        assert!(!dev_report.is_legacy);
        assert_eq!(dev_report.connection_info.host, "dev-db.example.com");
        assert_eq!(dev_report.connection_info.username, "dev_user");
        assert!(dev_report.connection_info.password_configured);

        let uat_report = status_report.get("uat").unwrap();
        assert_eq!(uat_report.name, "uat");
        assert_eq!(uat_report.status, EnvironmentStatus::Enabled);
        assert!(!uat_report.is_default);
    }

    #[test]
    fn test_disabled_environment() {
        let mut config = create_test_config_multi_env();
        
        // Disable UAT environment
        if let Some(environments) = &mut config.environments {
            if let Some(uat_env) = environments.get_mut("uat") {
                uat_env.enabled = false;
            }
        }

        let manager = EnvironmentManager::load_from_config(&config).unwrap();
        
        assert_eq!(manager.enabled_environment_count(), 1);
        let enabled_envs = manager.list_enabled_environments();
        assert_eq!(enabled_envs.len(), 1);
        assert!(enabled_envs.contains(&"dev"));
        assert!(!enabled_envs.contains(&"uat"));

        // Validation should fail for disabled environment
        assert!(manager.validate_environment("uat").is_err());
    }

    #[test]
    fn test_connection_url_methods() {
        let config = create_test_config_multi_env();
        let manager = EnvironmentManager::load_from_config(&config).unwrap();

        // Test masked connection URL
        let masked_url = manager.get_masked_connection_url("dev").unwrap();
        assert!(masked_url.contains("****"));
        assert!(!masked_url.contains("dev_password"));

        // Test full connection URL
        let full_url = manager.get_connection_url("dev").unwrap();
        assert!(full_url.contains("dev_password"));
        assert_eq!(full_url, "mysql://dev_user:dev_password@dev-db.example.com:3306/dev_db");
    }

    #[test]
    fn test_invalid_default_environment() {
        let mut config = create_test_config_multi_env();
        config.default_environment = Some("nonexistent".to_string());

        let result = EnvironmentManager::load_from_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_no_enabled_environments() {
        let mut config = create_test_config_multi_env();
        
        // Disable all environments
        if let Some(environments) = &mut config.environments {
            for env in environments.values_mut() {
                env.enabled = false;
            }
        }

        let result = EnvironmentManager::load_from_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_environment_metadata_access() {
        let config = create_test_config_multi_env();
        let manager = EnvironmentManager::load_from_config(&config).unwrap();

        let dev_metadata = manager.get_environment_metadata("dev").unwrap();
        assert_eq!(dev_metadata.config.name, "dev");
        assert_eq!(dev_metadata.status, EnvironmentStatus::Enabled);
        assert!(dev_metadata.is_default);

        let uat_metadata = manager.get_environment_metadata("uat").unwrap();
        assert_eq!(uat_metadata.config.name, "uat");
        assert_eq!(uat_metadata.status, EnvironmentStatus::Enabled);
        assert!(!uat_metadata.is_default);
    }

    #[test]
    fn test_has_environment() {
        let config = create_test_config_multi_env();
        let manager = EnvironmentManager::load_from_config(&config).unwrap();

        assert!(manager.has_environment("dev"));
        assert!(manager.has_environment("uat"));
        assert!(!manager.has_environment("prod"));
        assert!(!manager.has_environment("nonexistent"));
    }
}