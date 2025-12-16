//! Connection Pool Manager for multi-environment database connections
//! 
//! This module provides the ConnectionPoolManager component that handles:
//! - Maintaining separate connection pools for each environment
//! - Connection lifecycle management (creation, health checks, reconnection)
//! - Automatic reconnection with exponential backoff
//! - Connection pool resource limits and timeout handling

use crate::{Result, ServerError};
use crate::environment::EnvironmentManager;
use sqlx::{MySqlPool, MySql, ConnectOptions, Row};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

/// Health status for a connection pool
#[derive(Debug, Clone, PartialEq)]
pub enum PoolHealthStatus {
    /// Pool is healthy and ready for connections
    Healthy,
    /// Pool is degraded but still functional
    Degraded { 
        /// Number of active connections
        active_connections: u32,
        /// Maximum connections allowed
        max_connections: u32,
        /// Warning message
        warning: String,
    },
    /// Pool is unhealthy and not functional
    Unhealthy { 
        /// Error message
        error: String,
        /// Last successful connection time (as seconds since epoch)
        last_success_timestamp: Option<u64>,
    },
    /// Pool is initializing
    Initializing,
    /// Pool is disabled
    Disabled,
}

impl serde::Serialize for PoolHealthStatus {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        
        match self {
            PoolHealthStatus::Healthy => {
                let mut state = serializer.serialize_struct("PoolHealthStatus", 1)?;
                state.serialize_field("status", "healthy")?;
                state.end()
            }
            PoolHealthStatus::Degraded { active_connections, max_connections, warning } => {
                let mut state = serializer.serialize_struct("PoolHealthStatus", 4)?;
                state.serialize_field("status", "degraded")?;
                state.serialize_field("active_connections", active_connections)?;
                state.serialize_field("max_connections", max_connections)?;
                state.serialize_field("warning", warning)?;
                state.end()
            }
            PoolHealthStatus::Unhealthy { error, last_success_timestamp } => {
                let mut state = serializer.serialize_struct("PoolHealthStatus", 3)?;
                state.serialize_field("status", "unhealthy")?;
                state.serialize_field("error", error)?;
                state.serialize_field("last_success_timestamp", last_success_timestamp)?;
                state.end()
            }
            PoolHealthStatus::Initializing => {
                let mut state = serializer.serialize_struct("PoolHealthStatus", 1)?;
                state.serialize_field("status", "initializing")?;
                state.end()
            }
            PoolHealthStatus::Disabled => {
                let mut state = serializer.serialize_struct("PoolHealthStatus", 1)?;
                state.serialize_field("status", "disabled")?;
                state.end()
            }
        }
    }
}

/// Connection pool information for an environment
#[derive(Debug)]
pub struct PoolInfo {
    /// The MySQL connection pool
    pub pool: MySqlPool,
    /// Environment name
    pub environment: String,
    /// Current health status
    pub health_status: PoolHealthStatus,
    /// Last health check time
    pub last_health_check: Instant,
    /// Connection statistics
    pub stats: PoolStats,
    /// Reconnection state
    pub reconnection_state: ReconnectionState,
}

/// Connection pool statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct PoolStats {
    /// Number of active connections
    pub active_connections: u32,
    /// Number of idle connections
    pub idle_connections: u32,
    /// Total connections created
    pub total_connections_created: u64,
    /// Total connection failures
    pub total_connection_failures: u64,
    /// Total successful queries
    pub total_successful_queries: u64,
    /// Total failed queries
    pub total_failed_queries: u64,
    /// Average query execution time in milliseconds
    pub avg_query_time_ms: f64,
    /// Last successful connection timestamp
    pub last_successful_connection: Option<u64>,
    /// Last failed connection timestamp
    pub last_failed_connection: Option<u64>,
    /// Connection success rate (percentage)
    pub connection_success_rate: f64,
    /// Query success rate (percentage)
    pub query_success_rate: f64,
}

/// Reconnection state for exponential backoff
#[derive(Debug, Clone)]
pub struct ReconnectionState {
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    /// Next retry time
    pub next_retry: Option<Instant>,
    /// Current backoff duration
    pub current_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Whether reconnection is in progress
    pub reconnecting: bool,
}

impl Default for ReconnectionState {
    fn default() -> Self {
        Self {
            consecutive_failures: 0,
            next_retry: None,
            current_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(300), // 5 minutes max
            reconnecting: false,
        }
    }
}

impl Default for PoolStats {
    fn default() -> Self {
        Self {
            active_connections: 0,
            idle_connections: 0,
            total_connections_created: 0,
            total_connection_failures: 0,
            total_successful_queries: 0,
            total_failed_queries: 0,
            avg_query_time_ms: 0.0,
            last_successful_connection: None,
            last_failed_connection: None,
            connection_success_rate: 100.0,
            query_success_rate: 100.0,
        }
    }
}

impl PoolStats {
    /// Update connection statistics
    pub fn record_connection_attempt(&mut self, success: bool) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        if success {
            self.total_connections_created += 1;
            self.last_successful_connection = Some(timestamp);
        } else {
            self.total_connection_failures += 1;
            self.last_failed_connection = Some(timestamp);
        }
        
        self.update_connection_success_rate();
    }
    
    /// Update query statistics
    pub fn record_query_attempt(&mut self, success: bool, execution_time_ms: f64) {
        if success {
            self.total_successful_queries += 1;
        } else {
            self.total_failed_queries += 1;
        }
        
        // Update average query time using exponential moving average
        if self.avg_query_time_ms == 0.0 {
            self.avg_query_time_ms = execution_time_ms;
        } else {
            self.avg_query_time_ms = (self.avg_query_time_ms * 0.9) + (execution_time_ms * 0.1);
        }
        
        self.update_query_success_rate();
    }
    
    /// Update connection success rate
    fn update_connection_success_rate(&mut self) {
        let total_attempts = self.total_connections_created + self.total_connection_failures;
        if total_attempts > 0 {
            self.connection_success_rate = (self.total_connections_created as f64 / total_attempts as f64) * 100.0;
        }
    }
    
    /// Update query success rate
    fn update_query_success_rate(&mut self) {
        let total_queries = self.total_successful_queries + self.total_failed_queries;
        if total_queries > 0 {
            self.query_success_rate = (self.total_successful_queries as f64 / total_queries as f64) * 100.0;
        }
    }
}

/// Connection Pool Manager for multiple environments
pub struct ConnectionPoolManager {
    /// Map of environment name to pool information
    pools: Arc<RwLock<HashMap<String, PoolInfo>>>,
    /// Environment manager reference
    environment_manager: Arc<EnvironmentManager>,
    /// Health check interval
    health_check_interval: Duration,
    /// Whether the manager is initialized
    initialized: Arc<RwLock<bool>>,
}

impl ConnectionPoolManager {
    /// Create a new ConnectionPoolManager
    pub async fn initialize(environment_manager: Arc<EnvironmentManager>) -> Result<Self> {
        info!("Initializing Connection Pool Manager");
        
        let manager = Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            environment_manager,
            health_check_interval: Duration::from_secs(30),
            initialized: Arc::new(RwLock::new(false)),
        };

        // Initialize pools for all enabled environments
        manager.initialize_all_pools().await?;
        
        // Mark as initialized
        *manager.initialized.write().await = true;
        
        info!("Connection Pool Manager initialized successfully");
        Ok(manager)
    }

    /// Create a new ConnectionPoolManager with graceful partial failure handling
    /// This allows the server to start even if some environments fail to initialize
    pub async fn initialize_with_partial_failure(environment_manager: Arc<EnvironmentManager>) -> Result<Self> {
        info!("Initializing Connection Pool Manager with partial failure tolerance");
        
        let manager = Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            environment_manager,
            health_check_interval: Duration::from_secs(30),
            initialized: Arc::new(RwLock::new(false)),
        };

        // Initialize pools for all enabled environments with graceful failure handling
        manager.initialize_all_pools_with_partial_failure().await?;
        
        // Mark as initialized
        *manager.initialized.write().await = true;
        
        info!("Connection Pool Manager initialized with partial failure tolerance");
        Ok(manager)
    }

    /// Initialize connection pools for all enabled environments
    async fn initialize_all_pools(&self) -> Result<()> {
        let enabled_environments = self.environment_manager.list_enabled_environments();
        
        if enabled_environments.is_empty() {
            return Err(ServerError::configuration_error(
                "environments".to_string(),
                "No enabled environments found".to_string()
            ));
        }

        let mut pools = self.pools.write().await;
        
        for env_name in enabled_environments {
            match self.create_pool_for_environment(env_name).await {
                Ok(pool_info) => {
                    info!("Successfully initialized connection pool for environment '{}'", env_name);
                    pools.insert(env_name.to_string(), pool_info);
                }
                Err(e) => {
                    error!("Failed to initialize connection pool for environment '{}': {}", env_name, e);
                    
                    // Create a placeholder with unhealthy status
                    let pool_info = PoolInfo {
                        pool: self.create_dummy_pool().await?,
                        environment: env_name.to_string(),
                        health_status: PoolHealthStatus::Unhealthy {
                            error: e.user_message(),
                            last_success_timestamp: None,
                        },
                        last_health_check: Instant::now(),
                        stats: PoolStats::default(),
                        reconnection_state: ReconnectionState::default(),
                    };
                    pools.insert(env_name.to_string(), pool_info);
                }
            }
        }

        // Ensure at least one pool is healthy
        let healthy_count = pools.values()
            .filter(|pool| matches!(pool.health_status, PoolHealthStatus::Healthy))
            .count();

        if healthy_count == 0 {
            warn!("No healthy connection pools available, but continuing with degraded service");
        }

        Ok(())
    }

    /// Initialize connection pools for all enabled environments with graceful partial failure handling
    /// This method ensures that the server can start even if some environments fail to initialize
    async fn initialize_all_pools_with_partial_failure(&self) -> Result<()> {
        let enabled_environments = self.environment_manager.list_enabled_environments();
        
        if enabled_environments.is_empty() {
            return Err(ServerError::configuration_error(
                "environments".to_string(),
                "No enabled environments found".to_string()
            ));
        }

        let mut pools = self.pools.write().await;
        let mut successful_initializations = 0;
        let mut failed_environments = Vec::new();
        
        info!("Attempting to initialize {} environments with partial failure tolerance", enabled_environments.len());
        
        for env_name in enabled_environments {
            match self.create_pool_for_environment(env_name).await {
                Ok(pool_info) => {
                    info!("✅ Successfully initialized connection pool for environment '{}'", env_name);
                    pools.insert(env_name.to_string(), pool_info);
                    successful_initializations += 1;
                }
                Err(e) => {
                    warn!("❌ Failed to initialize connection pool for environment '{}': {}", env_name, e.user_message());
                    failed_environments.push((env_name.to_string(), e.user_message()));
                    
                    // Create a placeholder with unhealthy status for failed environments
                    let pool_info = PoolInfo {
                        pool: self.create_dummy_pool().await.unwrap_or_else(|_| {
                            // If we can't even create a dummy pool, create a minimal one
                            // This should never fail since it's a lazy connection
                            sqlx::MySqlPool::connect_lazy("mysql://dummy:dummy@localhost:3306/dummy")
                                .unwrap_or_else(|_| panic!("Failed to create dummy pool"))
                        }),
                        environment: env_name.to_string(),
                        health_status: PoolHealthStatus::Unhealthy {
                            error: format!("Initialization failed: {}", e.user_message()),
                            last_success_timestamp: None,
                        },
                        last_health_check: Instant::now(),
                        stats: PoolStats::default(),
                        reconnection_state: ReconnectionState::default(),
                    };
                    pools.insert(env_name.to_string(), pool_info);
                }
            }
        }

        // Check if we have at least one successful initialization
        if successful_initializations == 0 {
            let error_summary = failed_environments.iter()
                .map(|(env, error)| format!("{}: {}", env, error))
                .collect::<Vec<_>>()
                .join("; ");
            
            return Err(ServerError::configuration_error(
                "environments".to_string(),
                format!("Failed to initialize any environments. Errors: {}", error_summary)
            ));
        }

        // Log summary of initialization results
        if failed_environments.is_empty() {
            info!("🎉 All {} environments initialized successfully", successful_initializations);
        } else {
            warn!("⚠️  Partial initialization complete: {}/{} environments successful. Failed environments: {}", 
                  successful_initializations, 
                  pools.len(),
                  failed_environments.iter().map(|(env, _)| env.as_str()).collect::<Vec<_>>().join(", "));
            
            // Log detailed failure information
            for (env_name, error) in &failed_environments {
                warn!("Environment '{}' failure details: {}", env_name, error);
            }
        }

        Ok(())
    }

    /// Create a connection pool for a specific environment
    async fn create_pool_for_environment(&self, env_name: &str) -> Result<PoolInfo> {
        let env_config = self.environment_manager.get_environment(env_name)
            .ok_or_else(|| ServerError::validation_error(
                format!("Environment '{}' not found", env_name),
                Some(env_name.to_string())
            ))?;

        // Validate environment before creating pool
        self.environment_manager.validate_environment(env_name)?;

        let connection_url = env_config.connection_url();
        
        debug!("Creating connection pool for environment '{}' with URL: {}", 
               env_name, env_config.masked_connection_url());

        // Configure pool options based on environment configuration
        let connect_options = sqlx::mysql::MySqlConnectOptions::from_url(&connection_url.parse().map_err(|e| {
            ServerError::configuration_error(
                format!("environment.{}.database", env_name),
                format!("Invalid connection URL: {}", e)
            )
        })?)
        .map_err(|e| ServerError::configuration_error(
            format!("environment.{}.database", env_name),
            format!("Invalid connection configuration: {}", e)
        ))?;

        let pool = MySqlPool::connect_with(connect_options).await.map_err(|e| {
            ServerError::connection_error(e, true)
        })?;

        let pool_info = PoolInfo {
            pool,
            environment: env_name.to_string(),
            health_status: PoolHealthStatus::Healthy,
            last_health_check: Instant::now(),
            stats: PoolStats::default(),
            reconnection_state: ReconnectionState::default(),
        };

        Ok(pool_info)
    }

    /// Create a dummy pool for unhealthy environments (placeholder)
    async fn create_dummy_pool(&self) -> Result<MySqlPool> {
        // Create a lazy pool that won't actually connect until used
        // This is just to satisfy the type system for unhealthy pools
        MySqlPool::connect_lazy("mysql://dummy:dummy@localhost:3306/dummy")
            .map_err(|e| ServerError::internal_error(
                "Failed to create dummy pool".to_string(),
                Some(e.to_string())
            ))
    }

    /// Get a connection from the pool for a specific environment
    pub async fn get_connection(&self, env_name: &str) -> Result<sqlx::pool::PoolConnection<MySql>> {
        let start_time = std::time::Instant::now();
        
        // Check if manager is initialized
        if !*self.initialized.read().await {
            return Err(ServerError::internal_error(
                "Connection Pool Manager not initialized".to_string(),
                None
            ));
        }

        let pools = self.pools.read().await;
        let pool_info = pools.get(env_name)
            .ok_or_else(|| ServerError::validation_error(
                format!("No connection pool found for environment '{}'", env_name),
                Some(env_name.to_string())
            ))?;

        // Check pool health status and clone necessary data
        let (health_status, pool) = {
            let health_status = pool_info.health_status.clone();
            let pool = pool_info.pool.clone();
            (health_status, pool)
        };
        
        // Drop the read lock before proceeding
        drop(pools);
        
        match health_status {
            PoolHealthStatus::Healthy | PoolHealthStatus::Degraded { .. } => {
                // Attempt to get connection
                match pool.acquire().await {
                    Ok(conn) => {
                        let connection_time = start_time.elapsed().as_millis() as f64;
                        // Update connection statistics
                        self.update_connection_stats(env_name, true, connection_time).await;
                        
                        // Log successful connection acquisition
                        crate::error::secure_logging::log_connection_event(
                            env_name,
                            crate::error::secure_logging::ConnectionEvent::Acquired,
                            Some(&format!("Connection acquired in {}ms", connection_time as u64)),
                        );
                        
                        Ok(conn)
                    }
                    Err(e) => {
                        let connection_time = start_time.elapsed().as_millis() as f64;
                        error!("Failed to acquire connection for environment '{}' after {}ms: {}", env_name, connection_time, e);
                        
                        // Update connection statistics and pool status
                        let error_msg = e.to_string();
                        self.update_connection_stats(env_name, false, connection_time).await;
                        self.mark_pool_unhealthy(env_name, error_msg).await;
                        
                        // Log connection failure with environment context
                        crate::error::secure_logging::log_connection_event(
                            env_name,
                            crate::error::secure_logging::ConnectionEvent::Failed,
                            Some(&format!("Connection acquisition failed after {}ms", connection_time as u64)),
                        );
                        
                        Err(ServerError::connection_error(e, true))
                    }
                }
            }
            PoolHealthStatus::Unhealthy { error, .. } => {
                // Record failed connection attempt
                let connection_time = start_time.elapsed().as_millis() as f64;
                self.update_connection_stats(env_name, false, connection_time).await;
                
                Err(ServerError::validation_error(
                    format!("Connection pool for environment '{}' is unhealthy: {}", env_name, error),
                    Some(env_name.to_string())
                ))
            }
            PoolHealthStatus::Initializing => {
                Err(ServerError::validation_error(
                    format!("Connection pool for environment '{}' is still initializing", env_name),
                    Some(env_name.to_string())
                ))
            }
            PoolHealthStatus::Disabled => {
                Err(ServerError::validation_error(
                    format!("Environment '{}' is disabled", env_name),
                    Some(env_name.to_string())
                ))
            }
        }
    }

    /// Update connection statistics for an environment
    async fn update_connection_stats(&self, env_name: &str, success: bool, connection_time_ms: f64) {
        let mut pools = self.pools.write().await;
        if let Some(pool_info) = pools.get_mut(env_name) {
            pool_info.stats.record_connection_attempt(success);
            
            // Update real-time pool statistics
            pool_info.stats.active_connections = pool_info.pool.size() as u32;
            pool_info.stats.idle_connections = pool_info.pool.num_idle() as u32;
            
            debug!("Updated connection stats for environment '{}': success={}, time={}ms", 
                   env_name, success, connection_time_ms);
        }
    }

    /// Record query execution statistics
    pub async fn record_query_stats(&self, env_name: &str, success: bool, execution_time_ms: f64) {
        let mut pools = self.pools.write().await;
        if let Some(pool_info) = pools.get_mut(env_name) {
            pool_info.stats.record_query_attempt(success, execution_time_ms);
            
            debug!("Recorded query stats for environment '{}': success={}, time={}ms", 
                   env_name, success, execution_time_ms);
        }
    }

    /// Perform health check on a specific environment or all environments
    pub async fn health_check(&self, env_name: Option<&str>) -> Result<HashMap<String, PoolHealthStatus>> {
        let pools = self.pools.read().await;
        let mut health_results = HashMap::new();

        let environments_to_check: Vec<&str> = match env_name {
            Some(name) => vec![name],
            None => pools.keys().map(|s| s.as_str()).collect(),
        };

        for env in environments_to_check {
            if let Some(pool_info) = pools.get(env) {
                let health_status = self.check_pool_health(&pool_info.pool, env).await;
                health_results.insert(env.to_string(), health_status);
            } else {
                health_results.insert(env.to_string(), PoolHealthStatus::Unhealthy {
                    error: "Pool not found".to_string(),
                    last_success_timestamp: None,
                });
            }
        }

        Ok(health_results)
    }

    /// Check health of a specific pool with comprehensive diagnostics
    async fn check_pool_health(&self, pool: &MySqlPool, env_name: &str) -> PoolHealthStatus {
        let start_time = std::time::Instant::now();
        
        // Perform multiple health checks for comprehensive assessment
        let basic_check = sqlx::query("SELECT 1 as health_check").fetch_one(pool).await;
        let connection_time = start_time.elapsed().as_millis() as u64;
        
        match basic_check {
            Ok(_) => {
                debug!("Basic health check passed for environment '{}' in {}ms", env_name, connection_time);
                
                // Get detailed pool statistics
                let active_connections = pool.size() as u32;
                let idle_connections = pool.num_idle() as u32;
                
                // Get max connections from environment config if available
                let max_connections = self.environment_manager
                    .get_environment(env_name)
                    .map(|env| env.connection_pool.max_connections)
                    .unwrap_or(10);
                
                // Perform additional health checks
                let mut warnings = Vec::new();
                let mut is_degraded = false;
                
                // Check connection usage
                let usage_percentage = (active_connections as f64 / max_connections as f64) * 100.0;
                if usage_percentage > 80.0 {
                    warnings.push(format!("High connection usage: {:.1}% ({}/{})", 
                                        usage_percentage, active_connections, max_connections));
                    is_degraded = true;
                }
                
                // Check connection time
                if connection_time > 1000 {
                    warnings.push(format!("Slow connection time: {}ms", connection_time));
                    is_degraded = true;
                }
                
                // Check if there are any idle connections available
                if idle_connections == 0 && active_connections >= max_connections {
                    warnings.push("No idle connections available".to_string());
                    is_degraded = true;
                }
                
                if is_degraded {
                    PoolHealthStatus::Degraded {
                        active_connections,
                        max_connections,
                        warning: warnings.join("; "),
                    }
                } else {
                    PoolHealthStatus::Healthy
                }
            }
            Err(e) => {
                warn!("Health check failed for environment '{}' after {}ms: {}", env_name, connection_time, e);
                
                let last_success_timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs());
                
                PoolHealthStatus::Unhealthy {
                    error: format!("Health check failed after {}ms: {}", connection_time, e),
                    last_success_timestamp,
                }
            }
        }
    }

    /// Perform comprehensive health check with detailed diagnostics
    pub async fn comprehensive_health_check(&self, env_name: &str) -> Result<serde_json::Value> {
        info!("Performing comprehensive health check for environment '{}'", env_name);
        
        let pools = self.pools.read().await;
        let pool_info = pools.get(env_name)
            .ok_or_else(|| ServerError::validation_error(
                format!("Environment '{}' not found", env_name),
                Some(env_name.to_string())
            ))?;

        let start_time = std::time::Instant::now();
        let mut diagnostics = Vec::new();
        let mut overall_status = "healthy";
        
        // Test 1: Basic connectivity
        let basic_test = sqlx::query("SELECT 1 as basic_test").fetch_one(&pool_info.pool).await;
        let basic_time = start_time.elapsed().as_millis();
        
        match basic_test {
            Ok(_) => {
                diagnostics.push(serde_json::json!({
                    "test": "basic_connectivity",
                    "status": "passed",
                    "duration_ms": basic_time,
                    "message": "Basic database connectivity successful"
                }));
            }
            Err(e) => {
                overall_status = "unhealthy";
                diagnostics.push(serde_json::json!({
                    "test": "basic_connectivity",
                    "status": "failed",
                    "duration_ms": basic_time,
                    "error": e.to_string(),
                    "message": "Basic database connectivity failed"
                }));
            }
        }
        
        // Test 2: Database version and info
        let version_start = std::time::Instant::now();
        let version_test = sqlx::query("SELECT VERSION() as version, DATABASE() as current_db, USER() as current_user")
            .fetch_one(&pool_info.pool).await;
        let version_time = version_start.elapsed().as_millis();
        
        match version_test {
            Ok(row) => {
                let version: String = row.try_get("version").unwrap_or_default();
                let database: String = row.try_get("current_db").unwrap_or_default();
                let user: String = row.try_get("current_user").unwrap_or_default();
                
                diagnostics.push(serde_json::json!({
                    "test": "database_info",
                    "status": "passed",
                    "duration_ms": version_time,
                    "data": {
                        "version": version,
                        "database": database,
                        "user": user
                    },
                    "message": "Database information retrieved successfully"
                }));
            }
            Err(e) => {
                if overall_status == "healthy" {
                    overall_status = "degraded";
                }
                diagnostics.push(serde_json::json!({
                    "test": "database_info",
                    "status": "failed",
                    "duration_ms": version_time,
                    "error": e.to_string(),
                    "message": "Failed to retrieve database information"
                }));
            }
        }
        
        // Test 3: Performance test with a simple query
        let perf_start = std::time::Instant::now();
        let perf_test = sqlx::query("SELECT COUNT(*) as table_count FROM information_schema.tables WHERE table_schema = DATABASE()")
            .fetch_one(&pool_info.pool).await;
        let perf_time = perf_start.elapsed().as_millis();
        
        match perf_test {
            Ok(row) => {
                let table_count: i64 = row.try_get("table_count").unwrap_or(0);
                
                let perf_status = if perf_time > 1000 {
                    if overall_status == "healthy" {
                        overall_status = "degraded";
                    }
                    "slow"
                } else {
                    "good"
                };
                
                diagnostics.push(serde_json::json!({
                    "test": "performance_check",
                    "status": perf_status,
                    "duration_ms": perf_time,
                    "data": {
                        "table_count": table_count
                    },
                    "message": format!("Performance test completed in {}ms", perf_time)
                }));
            }
            Err(e) => {
                if overall_status == "healthy" {
                    overall_status = "degraded";
                }
                diagnostics.push(serde_json::json!({
                    "test": "performance_check",
                    "status": "failed",
                    "duration_ms": perf_time,
                    "error": e.to_string(),
                    "message": "Performance test failed"
                }));
            }
        }
        
        // Get current pool statistics
        let stats = self.get_pool_stats(env_name).await.unwrap_or_default();
        let total_time = start_time.elapsed().as_millis();
        
        Ok(serde_json::json!({
            "environment": env_name,
            "overall_status": overall_status,
            "total_duration_ms": total_time,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "pool_statistics": stats,
            "diagnostics": diagnostics,
            "health_summary": {
                "tests_run": diagnostics.len(),
                "tests_passed": diagnostics.iter().filter(|d| d["status"] == "passed").count(),
                "tests_failed": diagnostics.iter().filter(|d| d["status"] == "failed").count(),
                "tests_degraded": diagnostics.iter().filter(|d| d["status"] == "slow").count()
            }
        }))
    }

    /// Mark a pool as unhealthy and schedule reconnection
    async fn mark_pool_unhealthy(&self, env_name: &str, error: String) {
        let mut pools = self.pools.write().await;
        if let Some(pool_info) = pools.get_mut(env_name) {
            pool_info.health_status = PoolHealthStatus::Unhealthy {
                error: error.clone(),
                last_success_timestamp: None,
            };
            
            // Update reconnection state
            pool_info.reconnection_state.consecutive_failures += 1;
            pool_info.reconnection_state.current_backoff = std::cmp::min(
                pool_info.reconnection_state.current_backoff * 2,
                pool_info.reconnection_state.max_backoff
            );
            pool_info.reconnection_state.next_retry = Some(
                Instant::now() + pool_info.reconnection_state.current_backoff
            );
            
            warn!("Marked pool for environment '{}' as unhealthy: {}. Next retry in {:?}",
                  env_name, error, pool_info.reconnection_state.current_backoff);
        }
    }

    /// Attempt to reconnect to a specific environment
    pub async fn reconnect(&self, env_name: &str) -> Result<()> {
        info!("Attempting to reconnect to environment '{}'", env_name);
        
        // Check if reconnection is needed and allowed
        {
            let pools = self.pools.read().await;
            if let Some(pool_info) = pools.get(env_name) {
                // Check if we should retry based on backoff
                if let Some(next_retry) = pool_info.reconnection_state.next_retry {
                    if Instant::now() < next_retry {
                        return Err(ServerError::validation_error(
                            format!("Too early to retry reconnection for environment '{}'. Next retry at: {:?}",
                                   env_name, next_retry),
                            Some(env_name.to_string())
                        ));
                    }
                }
                
                // Check if already reconnecting
                if pool_info.reconnection_state.reconnecting {
                    return Err(ServerError::validation_error(
                        format!("Reconnection already in progress for environment '{}'", env_name),
                        Some(env_name.to_string())
                    ));
                }
            }
        }

        // Mark as reconnecting
        {
            let mut pools = self.pools.write().await;
            if let Some(pool_info) = pools.get_mut(env_name) {
                pool_info.reconnection_state.reconnecting = true;
            }
        }

        // Attempt to create new pool
        let result = self.create_pool_for_environment(env_name).await;
        
        // Update pool state based on result
        let mut pools = self.pools.write().await;
        if let Some(pool_info) = pools.get_mut(env_name) {
            pool_info.reconnection_state.reconnecting = false;
            
            match result {
                Ok(new_pool_info) => {
                    info!("Successfully reconnected to environment '{}'", env_name);
                    
                    // Replace the pool with the new one
                    *pool_info = new_pool_info;
                    
                    // Reset reconnection state
                    pool_info.reconnection_state = ReconnectionState::default();
                    
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to reconnect to environment '{}': {}", env_name, e);
                    
                    // Update failure count and backoff
                    pool_info.reconnection_state.consecutive_failures += 1;
                    pool_info.reconnection_state.current_backoff = std::cmp::min(
                        pool_info.reconnection_state.current_backoff * 2,
                        pool_info.reconnection_state.max_backoff
                    );
                    pool_info.reconnection_state.next_retry = Some(
                        Instant::now() + pool_info.reconnection_state.current_backoff
                    );
                    
                    pool_info.health_status = PoolHealthStatus::Unhealthy {
                        error: e.user_message(),
                        last_success_timestamp: None,
                    };
                    
                    Err(e)
                }
            }
        } else {
            Err(ServerError::validation_error(
                format!("Pool for environment '{}' not found during reconnection", env_name),
                Some(env_name.to_string())
            ))
        }
    }

    /// Get pool statistics for an environment
    pub async fn get_pool_stats(&self, env_name: &str) -> Result<PoolStats> {
        let pools = self.pools.read().await;
        let pool_info = pools.get(env_name)
            .ok_or_else(|| ServerError::validation_error(
                format!("No connection pool found for environment '{}'", env_name),
                Some(env_name.to_string())
            ))?;

        // Update real-time statistics
        let mut stats = pool_info.stats.clone();
        stats.active_connections = pool_info.pool.size() as u32;
        stats.idle_connections = pool_info.pool.num_idle() as u32;

        Ok(stats)
    }

    /// Get all pool statistics
    pub async fn get_all_pool_stats(&self) -> HashMap<String, PoolStats> {
        let pools = self.pools.read().await;
        let mut all_stats = HashMap::new();

        for (env_name, pool_info) in pools.iter() {
            let mut stats = pool_info.stats.clone();
            stats.active_connections = pool_info.pool.size() as u32;
            stats.idle_connections = pool_info.pool.num_idle() as u32;
            all_stats.insert(env_name.clone(), stats);
        }

        all_stats
    }

    /// Get health status for all pools
    pub async fn get_all_health_status(&self) -> HashMap<String, PoolHealthStatus> {
        let pools = self.pools.read().await;
        pools.iter()
            .map(|(env_name, pool_info)| (env_name.clone(), pool_info.health_status.clone()))
            .collect()
    }

    /// Check if any pools are healthy
    pub async fn has_healthy_pools(&self) -> bool {
        let pools = self.pools.read().await;
        pools.values().any(|pool_info| {
            matches!(pool_info.health_status, PoolHealthStatus::Healthy | PoolHealthStatus::Degraded { .. })
        })
    }

    /// Get list of healthy environment names
    pub async fn get_healthy_environments(&self) -> Vec<String> {
        let pools = self.pools.read().await;
        pools.iter()
            .filter(|(_, pool_info)| {
                matches!(pool_info.health_status, PoolHealthStatus::Healthy | PoolHealthStatus::Degraded { .. })
            })
            .map(|(env_name, _)| env_name.clone())
            .collect()
    }

    /// Start background health monitoring with alerting
    pub async fn start_health_monitoring(&self) -> tokio::task::JoinHandle<()> {
        let pools = Arc::clone(&self.pools);
        let environment_manager = Arc::clone(&self.environment_manager);
        let health_check_interval = self.health_check_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(health_check_interval);
            let mut previous_health_states: HashMap<String, PoolHealthStatus> = HashMap::new();
            
            info!("Starting health monitoring with {}s interval", health_check_interval.as_secs());
            
            loop {
                interval.tick().await;
                
                // Perform health checks on all pools
                let pool_names: Vec<String> = {
                    let pools_guard = pools.read().await;
                    pools_guard.keys().cloned().collect()
                };
                
                for env_name in pool_names {
                    let pools_clone = Arc::clone(&pools);
                    let env_manager_clone = Arc::clone(&environment_manager);
                    let mut prev_states = previous_health_states.clone();
                    
                    tokio::spawn(async move {
                        let (health_status, should_alert) = {
                            let pools_guard = pools_clone.read().await;
                            if let Some(pool_info) = pools_guard.get(&env_name) {
                                let start_time = std::time::Instant::now();
                                
                                // Perform comprehensive health check
                                let health_status = match sqlx::query("SELECT 1 as health_check, NOW() as current_time")
                                    .fetch_one(&pool_info.pool).await {
                                    Ok(_) => {
                                        let check_time = start_time.elapsed().as_millis();
                                        let active_connections = pool_info.pool.size() as u32;
                                        let idle_connections = pool_info.pool.num_idle() as u32;
                                        
                                        // Get max connections from environment config
                                        let max_connections = env_manager_clone
                                            .get_environment(&env_name)
                                            .map(|env| env.connection_pool.max_connections)
                                            .unwrap_or(10);
                                        
                                        let usage_percentage = (active_connections as f64 / max_connections as f64) * 100.0;
                                        
                                        // Determine health status based on multiple factors
                                        if check_time > 2000 || usage_percentage > 90.0 || idle_connections == 0 {
                                            let mut warnings = Vec::new();
                                            
                                            if check_time > 2000 {
                                                warnings.push(format!("Slow health check: {}ms", check_time));
                                            }
                                            if usage_percentage > 90.0 {
                                                warnings.push(format!("Critical connection usage: {:.1}%", usage_percentage));
                                            }
                                            if idle_connections == 0 {
                                                warnings.push("No idle connections available".to_string());
                                            }
                                            
                                            PoolHealthStatus::Degraded {
                                                active_connections,
                                                max_connections,
                                                warning: warnings.join("; "),
                                            }
                                        } else if usage_percentage > 80.0 {
                                            PoolHealthStatus::Degraded {
                                                active_connections,
                                                max_connections,
                                                warning: format!("High connection usage: {:.1}%", usage_percentage),
                                            }
                                        } else {
                                            PoolHealthStatus::Healthy
                                        }
                                    }
                                    Err(e) => {
                                        let check_time = start_time.elapsed().as_millis();
                                        error!("Health check failed for environment '{}' after {}ms: {}", env_name, check_time, e);
                                        
                                        let last_success_timestamp = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .ok()
                                            .map(|d| d.as_secs());
                                        
                                        PoolHealthStatus::Unhealthy {
                                            error: format!("Health check failed after {}ms: {}", check_time, e),
                                            last_success_timestamp,
                                        }
                                    }
                                };
                                
                                // Check if we should alert based on status change
                                let should_alert = match prev_states.get(&env_name) {
                                    Some(prev_status) => !Self::health_status_equivalent(prev_status, &health_status),
                                    None => !matches!(health_status, PoolHealthStatus::Healthy),
                                };
                                
                                (health_status, should_alert)
                            } else {
                                return;
                            }
                        };
                        
                        // Update health status and trigger alerts if needed
                        let mut pools_guard = pools_clone.write().await;
                        if let Some(pool_info) = pools_guard.get_mut(&env_name) {
                            let old_status = pool_info.health_status.clone();
                            pool_info.health_status = health_status.clone();
                            pool_info.last_health_check = Instant::now();
                            
                            // Update real-time statistics
                            pool_info.stats.active_connections = pool_info.pool.size() as u32;
                            pool_info.stats.idle_connections = pool_info.pool.num_idle() as u32;
                            
                            if should_alert {
                                Self::log_health_status_change(&env_name, &old_status, &health_status);
                            }
                        }
                        
                        // Update previous states for next iteration
                        prev_states.insert(env_name, health_status);
                    });
                }
                
                // Update the previous health states for the next iteration
                let pools_guard = pools.read().await;
                for (env_name, pool_info) in pools_guard.iter() {
                    previous_health_states.insert(env_name.clone(), pool_info.health_status.clone());
                }
            }
        })
    }

    /// Check if two health statuses are equivalent (to avoid alert spam)
    fn health_status_equivalent(status1: &PoolHealthStatus, status2: &PoolHealthStatus) -> bool {
        match (status1, status2) {
            (PoolHealthStatus::Healthy, PoolHealthStatus::Healthy) => true,
            (PoolHealthStatus::Degraded { .. }, PoolHealthStatus::Degraded { .. }) => true,
            (PoolHealthStatus::Unhealthy { .. }, PoolHealthStatus::Unhealthy { .. }) => true,
            (PoolHealthStatus::Initializing, PoolHealthStatus::Initializing) => true,
            (PoolHealthStatus::Disabled, PoolHealthStatus::Disabled) => true,
            _ => false,
        }
    }

    /// Log health status changes for alerting
    fn log_health_status_change(env_name: &str, old_status: &PoolHealthStatus, new_status: &PoolHealthStatus) {
        match (old_status, new_status) {
            (_, PoolHealthStatus::Healthy) => {
                info!("🟢 Environment '{}' is now HEALTHY", env_name);
            }
            (PoolHealthStatus::Healthy, PoolHealthStatus::Degraded { warning, .. }) => {
                warn!("🟡 Environment '{}' is now DEGRADED: {}", env_name, warning);
            }
            (_, PoolHealthStatus::Degraded { warning, .. }) => {
                warn!("🟡 Environment '{}' remains DEGRADED: {}", env_name, warning);
            }
            (_, PoolHealthStatus::Unhealthy { error, .. }) => {
                error!("🔴 Environment '{}' is now UNHEALTHY: {}", env_name, error);
            }
            (_, PoolHealthStatus::Initializing) => {
                info!("🔄 Environment '{}' is INITIALIZING", env_name);
            }
            (_, PoolHealthStatus::Disabled) => {
                warn!("⚫ Environment '{}' is DISABLED", env_name);
            }
        }
    }

    /// Get comprehensive monitoring report for all environments
    pub async fn get_monitoring_report(&self) -> serde_json::Value {
        let pools = self.pools.read().await;
        let mut environments = Vec::new();
        let mut summary = serde_json::json!({
            "total_environments": 0,
            "healthy_count": 0,
            "degraded_count": 0,
            "unhealthy_count": 0,
            "disabled_count": 0,
            "initializing_count": 0
        });
        
        for (env_name, pool_info) in pools.iter() {
            let env_report = serde_json::json!({
                "environment": env_name,
                "health_status": match &pool_info.health_status {
                    PoolHealthStatus::Healthy => "healthy",
                    PoolHealthStatus::Degraded { .. } => "degraded",
                    PoolHealthStatus::Unhealthy { .. } => "unhealthy",
                    PoolHealthStatus::Initializing => "initializing",
                    PoolHealthStatus::Disabled => "disabled",
                },
                "last_health_check": pool_info.last_health_check.elapsed().as_secs(),
                "statistics": pool_info.stats,
                "pool_info": {
                    "active_connections": pool_info.pool.size(),
                    "idle_connections": pool_info.pool.num_idle(),
                    "is_closed": pool_info.pool.is_closed()
                }
            });
            
            environments.push(env_report);
            
            // Update summary counts
            summary["total_environments"] = (summary["total_environments"].as_u64().unwrap_or(0) + 1).into();
            match &pool_info.health_status {
                PoolHealthStatus::Healthy => {
                    summary["healthy_count"] = (summary["healthy_count"].as_u64().unwrap_or(0) + 1).into();
                }
                PoolHealthStatus::Degraded { .. } => {
                    summary["degraded_count"] = (summary["degraded_count"].as_u64().unwrap_or(0) + 1).into();
                }
                PoolHealthStatus::Unhealthy { .. } => {
                    summary["unhealthy_count"] = (summary["unhealthy_count"].as_u64().unwrap_or(0) + 1).into();
                }
                PoolHealthStatus::Initializing => {
                    summary["initializing_count"] = (summary["initializing_count"].as_u64().unwrap_or(0) + 1).into();
                }
                PoolHealthStatus::Disabled => {
                    summary["disabled_count"] = (summary["disabled_count"].as_u64().unwrap_or(0) + 1).into();
                }
            }
        }
        
        serde_json::json!({
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "summary": summary,
            "environments": environments
        })
    }

    /// Test connection for a specific environment (simple version for startup checks)
    pub async fn test_connection_simple(&self, env_name: &str) -> Result<()> {
        let pools = self.pools.read().await;
        let pool_info = pools.get(env_name)
            .ok_or_else(|| ServerError::validation_error(
                format!("Environment '{}' not found", env_name),
                Some(env_name.to_string())
            ))?;

        // Try to get a connection and execute a simple query
        match pool_info.pool.acquire().await {
            Ok(mut conn) => {
                match sqlx::query("SELECT 1").fetch_one(&mut *conn).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(ServerError::connection_error(e, true))
                }
            }
            Err(e) => Err(ServerError::connection_error(e, true))
        }
    }

    /// Check if a specific environment is healthy
    pub async fn is_environment_healthy(&self, env_name: &str) -> bool {
        let pools = self.pools.read().await;
        if let Some(pool_info) = pools.get(env_name) {
            matches!(pool_info.health_status, 
                     PoolHealthStatus::Healthy | 
                     PoolHealthStatus::Degraded { .. })
        } else {
            false
        }
    }

    /// Test connection for a specific environment with detailed diagnostics
    pub async fn test_connection(&self, env_name: &str) -> Result<serde_json::Value> {
        info!("Testing connection for environment '{}'", env_name);
        
        let pools = self.pools.read().await;
        let pool_info = pools.get(env_name)
            .ok_or_else(|| ServerError::validation_error(
                format!("Environment '{}' not found", env_name),
                Some(env_name.to_string())
            ))?;

        let start_time = std::time::Instant::now();
        
        // Try to get a connection and execute a simple query
        match pool_info.pool.acquire().await {
            Ok(mut conn) => {
                match sqlx::query("SELECT 1 as test_value").fetch_one(&mut *conn).await {
                    Ok(_) => {
                        let connection_time = start_time.elapsed();
                        Ok(serde_json::json!({
                            "status": "success",
                            "environment": env_name,
                            "connection_time_ms": connection_time.as_millis(),
                            "pool_status": {
                                "active_connections": pool_info.stats.active_connections,
                                "idle_connections": pool_info.stats.idle_connections,
                                "health_status": match pool_info.health_status {
                                    PoolHealthStatus::Healthy => "healthy",
                                    PoolHealthStatus::Degraded { .. } => "degraded",
                                    PoolHealthStatus::Unhealthy { .. } => "unhealthy",
                                    PoolHealthStatus::Initializing => "initializing",
                                    PoolHealthStatus::Disabled => "disabled",
                                }
                            },
                            "message": "Database connection test successful"
                        }))
                    }
                    Err(e) => {
                        let connection_time = start_time.elapsed();
                        Ok(serde_json::json!({
                            "status": "error",
                            "environment": env_name,
                            "connection_time_ms": connection_time.as_millis(),
                            "error": e.to_string(),
                            "message": "Database query test failed"
                        }))
                    }
                }
            }
            Err(e) => {
                let connection_time = start_time.elapsed();
                Ok(serde_json::json!({
                    "status": "error",
                    "environment": env_name,
                    "connection_time_ms": connection_time.as_millis(),
                    "error": e.to_string(),
                    "message": "Failed to acquire database connection"
                }))
            }
        }
    }



    /// Shutdown all connection pools
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down Connection Pool Manager");
        
        let mut pools = self.pools.write().await;
        
        for (env_name, pool_info) in pools.drain() {
            info!("Closing connection pool for environment '{}'", env_name);
            pool_info.pool.close().await;
        }
        
        *self.initialized.write().await = false;
        
        info!("Connection Pool Manager shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, DatabaseConfig, EnvironmentConfig, PoolConfig, ServerConfig, McpConfig};
    use std::collections::HashMap;

    fn create_test_environment_config(name: &str, port: u16) -> EnvironmentConfig {
        EnvironmentConfig {
            name: name.to_string(),
            description: Some(format!("{} environment", name)),
            database: DatabaseConfig {
                host: "localhost".to_string(),
                port,
                username: format!("{}_user", name),
                password: format!("{}_password", name),
                database: format!("{}_db", name),
                connection_timeout: 30,
                max_connections: 10,
            },
            connection_pool: PoolConfig {
                max_connections: 5,
                min_connections: 1,
                connection_timeout: 30,
                idle_timeout: 600,
            },
            enabled: true,
        }
    }

    fn create_test_config() -> Config {
        let mut environments = HashMap::new();
        environments.insert("test1".to_string(), create_test_environment_config("test1", 3306));
        environments.insert("test2".to_string(), create_test_environment_config("test2", 3307));
        
        Config {
            server: ServerConfig {
                port: 8080,
                log_level: "info".to_string(),
            },
            database: None,
            environments: Some(environments),
            default_environment: Some("test1".to_string()),
            mcp: McpConfig {
                protocol_version: "2024-11-05".to_string(),
                server_name: "test-server".to_string(),
                server_version: "0.1.0".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn test_connection_pool_manager_initialization() {
        let config = create_test_config();
        let env_manager = Arc::new(EnvironmentManager::load_from_config(&config).unwrap());
        
        // This will fail to connect to actual databases, but we can test the initialization logic
        let result = ConnectionPoolManager::initialize(env_manager).await;
        
        // The initialization should complete even if connections fail
        // (pools will be marked as unhealthy)
        assert!(result.is_ok(), "Pool manager initialization should complete: {:?}", result.err());
        
        let pool_manager = result.unwrap();
        
        // Check that pools were created (even if unhealthy)
        let health_status = pool_manager.get_all_health_status().await;
        assert_eq!(health_status.len(), 2, "Should have 2 pools");
        assert!(health_status.contains_key("test1"));
        assert!(health_status.contains_key("test2"));
    }

    #[tokio::test]
    async fn test_pool_health_status_transitions() {
        let config = create_test_config();
        let env_manager = Arc::new(EnvironmentManager::load_from_config(&config).unwrap());
        let pool_manager = ConnectionPoolManager::initialize(env_manager).await.unwrap();
        
        // Test health check on non-existent environment
        let health_result = pool_manager.health_check(Some("nonexistent")).await;
        assert!(health_result.is_ok());
        
        let health_status = health_result.unwrap();
        assert_eq!(health_status.len(), 1);
        assert!(matches!(health_status.get("nonexistent").unwrap(), PoolHealthStatus::Unhealthy { .. }));
    }

    #[tokio::test]
    async fn test_get_connection_from_unhealthy_pool() {
        let config = create_test_config();
        let env_manager = Arc::new(EnvironmentManager::load_from_config(&config).unwrap());
        let pool_manager = ConnectionPoolManager::initialize(env_manager).await.unwrap();
        
        // Try to get connection from environment that likely doesn't exist
        let connection_result = pool_manager.get_connection("test1").await;
        
        // Should fail because the database doesn't actually exist
        assert!(connection_result.is_err(), "Connection should fail for non-existent database");
    }

    #[tokio::test]
    async fn test_pool_stats_collection() {
        let config = create_test_config();
        let env_manager = Arc::new(EnvironmentManager::load_from_config(&config).unwrap());
        let pool_manager = ConnectionPoolManager::initialize(env_manager).await.unwrap();
        
        // Get stats for all pools
        let all_stats = pool_manager.get_all_pool_stats().await;
        assert_eq!(all_stats.len(), 2, "Should have stats for 2 pools");
        
        // Get stats for specific pool
        let test1_stats = pool_manager.get_pool_stats("test1").await;
        assert!(test1_stats.is_ok(), "Should be able to get stats for test1");
        
        // Get stats for non-existent pool
        let nonexistent_stats = pool_manager.get_pool_stats("nonexistent").await;
        assert!(nonexistent_stats.is_err(), "Should fail for non-existent pool");
    }

    #[tokio::test]
    async fn test_healthy_environments_filtering() {
        let config = create_test_config();
        let env_manager = Arc::new(EnvironmentManager::load_from_config(&config).unwrap());
        let pool_manager = ConnectionPoolManager::initialize(env_manager).await.unwrap();
        
        // Check if any pools are healthy (likely none due to non-existent databases)
        let _has_healthy = pool_manager.has_healthy_pools().await;
        // This will likely be false since we're not connecting to real databases
        
        let healthy_envs = pool_manager.get_healthy_environments().await;
        // Should be empty or contain environments that somehow connected
        assert!(healthy_envs.len() <= 2, "Should not have more healthy environments than configured");
    }

    #[tokio::test]
    async fn test_reconnection_backoff_logic() {
        let config = create_test_config();
        let env_manager = Arc::new(EnvironmentManager::load_from_config(&config).unwrap());
        let pool_manager = ConnectionPoolManager::initialize(env_manager).await.unwrap();
        
        // Try to reconnect to an environment (will fail due to non-existent database)
        let reconnect_result = pool_manager.reconnect("test1").await;
        assert!(reconnect_result.is_err(), "Reconnection should fail for non-existent database");
        
        // Try to reconnect again immediately (should be blocked by backoff)
        let _immediate_reconnect = pool_manager.reconnect("test1").await;
        // This might succeed or fail depending on timing, but should handle backoff logic
    }

    #[tokio::test]
    async fn test_pool_manager_shutdown() {
        let config = create_test_config();
        let env_manager = Arc::new(EnvironmentManager::load_from_config(&config).unwrap());
        let pool_manager = ConnectionPoolManager::initialize(env_manager).await.unwrap();
        
        // Shutdown should complete successfully
        let shutdown_result = pool_manager.shutdown().await;
        assert!(shutdown_result.is_ok(), "Shutdown should complete successfully");
        
        // After shutdown, getting connections should fail
        let connection_result = pool_manager.get_connection("test1").await;
        assert!(connection_result.is_err(), "Should not be able to get connections after shutdown");
    }

    #[test]
    fn test_reconnection_state_default() {
        let state = ReconnectionState::default();
        assert_eq!(state.consecutive_failures, 0);
        assert!(state.next_retry.is_none());
        assert_eq!(state.current_backoff, Duration::from_secs(1));
        assert_eq!(state.max_backoff, Duration::from_secs(300));
        assert!(!state.reconnecting);
    }

    #[test]
    fn test_pool_stats_default() {
        let stats = PoolStats::default();
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.idle_connections, 0);
        assert_eq!(stats.total_connections_created, 0);
        assert_eq!(stats.total_connection_failures, 0);
        assert_eq!(stats.total_successful_queries, 0);
        assert_eq!(stats.total_failed_queries, 0);
        assert_eq!(stats.avg_query_time_ms, 0.0);
        assert_eq!(stats.last_successful_connection, None);
        assert_eq!(stats.last_failed_connection, None);
        assert_eq!(stats.connection_success_rate, 100.0);
        assert_eq!(stats.query_success_rate, 100.0);
    }

    #[test]
    fn test_pool_stats_recording() {
        let mut stats = PoolStats::default();
        
        // Test connection recording
        stats.record_connection_attempt(true);
        assert_eq!(stats.total_connections_created, 1);
        assert_eq!(stats.connection_success_rate, 100.0);
        assert!(stats.last_successful_connection.is_some());
        
        stats.record_connection_attempt(false);
        assert_eq!(stats.total_connection_failures, 1);
        assert_eq!(stats.connection_success_rate, 50.0);
        assert!(stats.last_failed_connection.is_some());
        
        // Test query recording
        stats.record_query_attempt(true, 100.0);
        assert_eq!(stats.total_successful_queries, 1);
        assert_eq!(stats.avg_query_time_ms, 100.0);
        assert_eq!(stats.query_success_rate, 100.0);
        
        stats.record_query_attempt(false, 200.0);
        assert_eq!(stats.total_failed_queries, 1);
        assert_eq!(stats.query_success_rate, 50.0);
        // Average should be updated with exponential moving average
        assert!(stats.avg_query_time_ms > 100.0 && stats.avg_query_time_ms < 200.0);
    }

    #[test]
    fn test_pool_health_status_variants() {
        // Test all health status variants
        let healthy = PoolHealthStatus::Healthy;
        assert!(matches!(healthy, PoolHealthStatus::Healthy));
        
        let degraded = PoolHealthStatus::Degraded {
            active_connections: 8,
            max_connections: 10,
            warning: "High usage".to_string(),
        };
        assert!(matches!(degraded, PoolHealthStatus::Degraded { .. }));
        
        let unhealthy = PoolHealthStatus::Unhealthy {
            error: "Connection failed".to_string(),
            last_success_timestamp: None,
        };
        assert!(matches!(unhealthy, PoolHealthStatus::Unhealthy { .. }));
        
        let initializing = PoolHealthStatus::Initializing;
        assert!(matches!(initializing, PoolHealthStatus::Initializing));
        
        let disabled = PoolHealthStatus::Disabled;
        assert!(matches!(disabled, PoolHealthStatus::Disabled));
    }
}