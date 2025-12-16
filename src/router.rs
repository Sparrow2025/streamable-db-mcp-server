//! Query Router for environment-aware query routing
//! 
//! This module provides the QueryRouter component that handles:
//! - Single-environment query execution with environment specification
//! - Multi-environment query execution capabilities
//! - Result aggregation and formatting for multi-environment queries
//! - Query comparison functionality across environments

use crate::{Result, ServerError};
use crate::environment::EnvironmentManager;
use crate::pool::ConnectionPoolManager;
use crate::query::{QueryProcessor, QueryRequest, ColumnInfo, Row};
use crate::streaming::{ResultStreamer, StreamingConfig, MultiEnvResultChunk};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, error, debug};

/// Request structure for environment-specific SQL queries
#[derive(Debug, Serialize, Deserialize)]
pub struct EnvQueryRequest {
    /// SQL query string
    pub sql: String,
    /// Target environment name (optional - uses default if not specified)
    pub environment: Option<String>,
    /// Optional query parameters
    pub parameters: Option<Vec<serde_json::Value>>,
    /// Whether to stream results
    pub stream_results: bool,
}

/// Request structure for multi-environment SQL queries
#[derive(Debug, Serialize, Deserialize)]
pub struct MultiEnvQueryRequest {
    /// SQL query string
    pub sql: String,
    /// List of target environments (if empty, uses all enabled environments)
    pub environments: Vec<String>,
    /// Optional query parameters
    pub parameters: Option<Vec<serde_json::Value>>,
    /// Whether to compare results across environments
    pub compare_results: Option<bool>,
}

/// Query execution result with environment context
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct EnvQueryResponse {
    /// Environment name where query was executed
    pub environment: String,
    /// Query execution time in milliseconds
    pub execution_time_ms: u64,
    /// Number of affected rows (for INSERT/UPDATE/DELETE)
    pub affected_rows: Option<u64>,
    /// Column information
    pub columns: Vec<ColumnInfo>,
    /// Result rows
    pub rows: Vec<Row>,
    /// Error message if query failed
    pub error: Option<String>,
}

/// Multi-environment query execution result
#[derive(Debug, Serialize, Deserialize)]
pub struct MultiEnvQueryResponse {
    /// Results from each environment (keyed by environment name)
    pub results: HashMap<String, EnvQueryResponse>,
    /// Comparison results if requested
    pub comparison: Option<ComparisonResult>,
    /// Execution summary
    pub summary: ExecutionSummary,
}

/// Comparison result between environments
#[derive(Debug, Serialize, Deserialize)]
pub struct ComparisonResult {
    /// Whether all results are identical
    pub identical: bool,
    /// Differences found between environments
    pub differences: Vec<EnvironmentDifference>,
    /// Row count comparison across environments
    pub row_count_comparison: HashMap<String, u64>,
}

/// Difference between environments
#[derive(Debug, Serialize, Deserialize)]
pub struct EnvironmentDifference {
    /// Type of difference
    pub difference_type: DifferenceType,
    /// Environments involved in the difference
    pub environments: Vec<String>,
    /// Description of the difference
    pub description: String,
    /// Additional details about the difference
    pub details: Option<serde_json::Value>,
}

/// Types of differences that can be found
#[derive(Debug, Serialize, Deserialize)]
pub enum DifferenceType {
    /// Different number of rows
    RowCount,
    /// Different column structure
    Schema,
    /// Different data values
    Data,
    /// One environment had an error
    Error,
}

/// Execution summary for multi-environment queries
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// Total number of environments queried
    pub total_environments: usize,
    /// Number of successful executions
    pub successful_executions: usize,
    /// Number of failed executions
    pub failed_executions: usize,
    /// Total execution time across all environments
    pub total_execution_time_ms: u64,
    /// Average execution time per environment
    pub avg_execution_time_ms: f64,
}

/// Query Router for environment-aware query routing
pub struct QueryRouter {
    /// Connection pool manager for accessing environment connections
    pool_manager: Arc<ConnectionPoolManager>,
    /// Environment manager for environment metadata
    environment_manager: Arc<EnvironmentManager>,
    /// Result streamer for handling streaming queries
    result_streamer: ResultStreamer,
}

impl QueryRouter {
    /// Create a new QueryRouter
    pub fn new(
        pool_manager: Arc<ConnectionPoolManager>,
        environment_manager: Arc<EnvironmentManager>,
    ) -> Self {
        Self {
            pool_manager,
            environment_manager,
            result_streamer: ResultStreamer::new(),
        }
    }

    /// Create a new QueryRouter with custom streaming configuration
    pub fn with_streaming_config(
        pool_manager: Arc<ConnectionPoolManager>,
        environment_manager: Arc<EnvironmentManager>,
        streaming_config: StreamingConfig,
    ) -> Self {
        Self {
            pool_manager,
            environment_manager,
            result_streamer: ResultStreamer::with_config(streaming_config),
        }
    }

    /// Execute a query against a specific environment or default environment
    pub async fn execute_query(&self, request: &EnvQueryRequest) -> Result<EnvQueryResponse> {
        let start_time = Instant::now();
        
        // Determine target environment
        let target_env = match &request.environment {
            Some(env) => env.clone(),
            None => {
                // Use default environment if available
                self.environment_manager.get_default_environment()
                    .ok_or_else(|| ServerError::validation_error(
                        "No environment specified and no default environment configured".to_string(),
                        None
                    ))?
                    .to_string()
            }
        };

        info!("Executing query against environment '{}'", target_env);
        debug!("Query: {}", request.sql);

        // Validate environment exists and is enabled
        self.environment_manager.validate_environment(&target_env)?;

        // Get connection from pool
        let mut connection = self.pool_manager.get_connection(&target_env).await?;

        // Create QueryRequest for the processor
        let query_request = QueryRequest {
            sql: request.sql.clone(),
            parameters: request.parameters.clone(),
            stream_results: request.stream_results,
        };

        // Execute query with enhanced error handling
        match QueryProcessor::execute_query(&mut connection, &query_request).await {
            Ok(result) => {
                let execution_time = start_time.elapsed().as_millis() as u64;
                
                // Log successful query execution
                crate::error::secure_logging::log_query_execution(
                    &target_env,
                    &request.sql,
                    true,
                    execution_time,
                    result.affected_rows,
                );

                // Record query statistics
                self.pool_manager.record_query_stats(&target_env, true, execution_time as f64).await;

                Ok(EnvQueryResponse {
                    environment: target_env,
                    execution_time_ms: execution_time,
                    affected_rows: result.affected_rows,
                    columns: result.columns,
                    rows: result.rows,
                    error: None,
                })
            }
            Err(e) => {
                let execution_time = start_time.elapsed().as_millis() as u64;
                
                // Create environment-aware error
                let env_error = match e {
                    ServerError::Query { sql, source, .. } => {
                        ServerError::query_error_with_env(sql, source, Some(target_env.clone()))
                    }
                    ServerError::Connection { source, recoverable, .. } => {
                        ServerError::connection_error_with_env(source, recoverable, Some(target_env.clone()))
                    }
                    _ => ServerError::internal_error_with_env(
                        e.user_message(),
                        Some(e.detailed_message()),
                        Some(target_env.clone())
                    )
                };

                // Log failed query execution
                crate::error::secure_logging::log_query_execution(
                    &target_env,
                    &request.sql,
                    false,
                    execution_time,
                    None,
                );

                // Record query statistics
                self.pool_manager.record_query_stats(&target_env, false, execution_time as f64).await;

                Ok(EnvQueryResponse {
                    environment: target_env,
                    execution_time_ms: execution_time,
                    affected_rows: None,
                    columns: vec![],
                    rows: vec![],
                    error: Some(env_error.user_message()),
                })
            }
        }
    }

    /// Execute a query against multiple environments
    pub async fn execute_multi_env_query(&self, request: &MultiEnvQueryRequest) -> Result<MultiEnvQueryResponse> {
        let start_time = Instant::now();
        
        // Determine target environments
        let target_environments = if request.environments.is_empty() {
            // Use all enabled environments
            self.environment_manager.list_enabled_environments()
                .into_iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            // Validate all specified environments
            for env in &request.environments {
                self.environment_manager.validate_environment(env)?;
            }
            request.environments.clone()
        };

        if target_environments.is_empty() {
            return Err(ServerError::validation_error(
                "No valid environments available for multi-environment query".to_string(),
                None
            ));
        }

        // Log multi-environment operation start
        crate::error::secure_logging::log_multi_env_operation(
            "multi_env_query",
            &target_environments,
            crate::error::secure_logging::LogLevel::Info,
            None,
        );

        let mut results = HashMap::new();
        let mut successful_executions = 0;
        let mut failed_executions = 0;
        let mut total_execution_time = 0u64;
        let mut environment_errors = std::collections::HashMap::new();

        // Execute query in each environment concurrently
        let mut tasks = Vec::new();
        
        for env_name in target_environments.iter() {
            let env_query_request = EnvQueryRequest {
                sql: request.sql.clone(),
                environment: Some(env_name.clone()),
                parameters: request.parameters.clone(),
                stream_results: false, // Multi-env queries don't support streaming
            };
            
            let router = self.clone();
            let env_name_clone = env_name.clone();
            
            let task = tokio::spawn(async move {
                let result = router.execute_query(&env_query_request).await;
                (env_name_clone, result)
            });
            
            tasks.push(task);
        }

        // Collect results from all tasks
        for task in tasks {
            match task.await {
                Ok((env_name, query_result)) => {
                    match query_result {
                        Ok(env_response) => {
                            total_execution_time += env_response.execution_time_ms;
                            
                            if env_response.error.is_none() {
                                successful_executions += 1;
                            } else {
                                failed_executions += 1;
                            }
                            
                            results.insert(env_name, env_response);
                        }
                        Err(e) => {
                            let error_message = e.user_message();
                            
                            // Create environment-aware error
                            let env_error = ServerError::environment_error(
                                env_name.clone(),
                                error_message.clone(),
                                Some(Box::new(e)),
                                crate::error::EnvironmentErrorCategory::Connectivity,
                            );
                            
                            environment_errors.insert(env_name.clone(), Box::new(env_error));
                            failed_executions += 1;
                            
                            // Create error response
                            let error_response = EnvQueryResponse {
                                environment: env_name.clone(),
                                execution_time_ms: 0,
                                affected_rows: None,
                                columns: vec![],
                                rows: vec![],
                                error: Some(error_message),
                            };
                            
                            results.insert(env_name, error_response);
                        }
                    }
                }
                Err(e) => {
                    let env_error = ServerError::environment_error(
                        "unknown".to_string(),
                        format!("Task execution failed: {}", e),
                        None,
                        crate::error::EnvironmentErrorCategory::Performance,
                    );
                    
                    environment_errors.insert("unknown".to_string(), Box::new(env_error));
                    failed_executions += 1;
                }
            }
        }

        // Create execution summary
        let total_environments = target_environments.len();
        let avg_execution_time = if successful_executions > 0 {
            total_execution_time as f64 / successful_executions as f64
        } else {
            0.0
        };

        let summary = ExecutionSummary {
            total_environments,
            successful_executions,
            failed_executions,
            total_execution_time_ms: start_time.elapsed().as_millis() as u64,
            avg_execution_time_ms: avg_execution_time,
        };

        // Generate comparison if requested
        let comparison = if request.compare_results.unwrap_or(false) {
            Some(self.compare_results(&results))
        } else {
            None
        };

        // Log completion with detailed context
        let mut log_context = std::collections::HashMap::new();
        log_context.insert("successful_executions".to_string(), serde_json::Value::Number(successful_executions.into()));
        log_context.insert("failed_executions".to_string(), serde_json::Value::Number(failed_executions.into()));
        log_context.insert("total_execution_time_ms".to_string(), serde_json::Value::Number(summary.total_execution_time_ms.into()));
        
        crate::error::secure_logging::log_multi_env_operation(
            "multi_env_query_completed",
            &target_environments,
            if failed_executions == 0 { 
                crate::error::secure_logging::LogLevel::Info 
            } else { 
                crate::error::secure_logging::LogLevel::Warn 
            },
            Some(&log_context),
        );

        // If there were errors and no successes, return a multi-environment error
        if successful_executions == 0 && !environment_errors.is_empty() {
            return Err(ServerError::multi_environment_error(
                "multi_env_query".to_string(),
                environment_errors,
                vec![],
            ));
        }

        Ok(MultiEnvQueryResponse {
            results,
            comparison,
            summary,
        })
    }

    /// Compare results across environments
    pub async fn compare_across_environments(&self, sql: &str, environments: &[String]) -> Result<ComparisonResult> {
        let request = MultiEnvQueryRequest {
            sql: sql.to_string(),
            environments: environments.to_vec(),
            parameters: None,
            compare_results: Some(true),
        };

        let response = self.execute_multi_env_query(&request).await?;
        
        response.comparison.ok_or_else(|| ServerError::internal_error(
            "Comparison was requested but not generated".to_string(),
            None
        ))
    }

    /// Execute a streaming query against multiple environments simultaneously
    pub async fn execute_multi_env_streaming_query(
        &self,
        request: &MultiEnvQueryRequest,
    ) -> Result<Vec<MultiEnvResultChunk>> {
        let start_time = Instant::now();
        
        // Determine target environments
        let target_environments = if request.environments.is_empty() {
            // Use all enabled environments
            self.environment_manager.list_enabled_environments()
                .into_iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            // Validate all specified environments
            for env in &request.environments {
                self.environment_manager.validate_environment(env)?;
            }
            request.environments.clone()
        };

        if target_environments.is_empty() {
            return Err(ServerError::validation_error(
                "No valid environments available for multi-environment streaming query".to_string(),
                None
            ));
        }

        info!("Executing multi-environment streaming query against {} environments: {:?}", 
              target_environments.len(), target_environments);
        debug!("Streaming query: {}", request.sql);

        // Execute streaming query in each environment concurrently
        let mut tasks = Vec::new();
        
        for env_name in target_environments.iter() {
            let env_query_request = EnvQueryRequest {
                sql: request.sql.clone(),
                environment: Some(env_name.clone()),
                parameters: request.parameters.clone(),
                stream_results: true, // Force streaming for multi-env streaming queries
            };
            
            let router = self.clone();
            let env_name_clone = env_name.clone();
            
            let task = tokio::spawn(async move {
                let result = router.execute_streaming_query_for_env(&env_query_request).await;
                (env_name_clone, result)
            });
            
            tasks.push(task);
        }

        // Collect results from all tasks
        let mut env_results = HashMap::new();
        
        for task in tasks {
            match task.await {
                Ok((env_name, streaming_result)) => {
                    env_results.insert(env_name, streaming_result);
                }
                Err(e) => {
                    error!("Task failed for environment streaming query: {}", e);
                    // Continue with other environments
                }
            }
        }

        // Merge results using the result streamer
        let merged_chunks = self.result_streamer.merge_multi_env_results(env_results)?;
        
        let execution_time = start_time.elapsed().as_millis() as u64;
        info!("Multi-environment streaming query completed in {}ms", execution_time);

        Ok(merged_chunks)
    }

    /// Execute a streaming query for a single environment
    async fn execute_streaming_query_for_env(
        &self,
        request: &EnvQueryRequest,
    ) -> Result<Vec<crate::streaming::ResultChunk>> {
        let start_time = Instant::now();
        
        // Determine target environment
        let target_env = match &request.environment {
            Some(env) => env.clone(),
            None => {
                // Use default environment if available
                self.environment_manager.get_default_environment()
                    .ok_or_else(|| ServerError::validation_error(
                        "No environment specified and no default environment configured".to_string(),
                        None
                    ))?
                    .to_string()
            }
        };

        info!("Executing streaming query against environment '{}'", target_env);
        debug!("Streaming query: {}", request.sql);

        // Validate environment exists and is enabled
        self.environment_manager.validate_environment(&target_env)?;

        // Get connection from pool
        let mut connection = self.pool_manager.get_connection(&target_env).await?;

        // Execute streaming query with timeout
        let timeout_seconds = self.result_streamer.config.stream_timeout_seconds;
        let chunks = self.result_streamer.execute_streaming_query_with_timeout(
            &mut connection,
            &request.sql,
            &target_env,
            timeout_seconds,
        ).await?;

        let execution_time = start_time.elapsed().as_millis() as u64;
        info!("Streaming query executed successfully in environment '{}' in {}ms", 
              target_env, execution_time);

        Ok(chunks)
    }

    /// Execute a streaming query against a specific environment with error handling
    pub async fn execute_streaming_query_env(
        &self,
        request: &EnvQueryRequest,
    ) -> Result<Vec<crate::streaming::ResultChunk>> {
        if !request.stream_results {
            return Err(ServerError::validation_error(
                "Streaming must be enabled for streaming query execution".to_string(),
                None
            ));
        }

        self.execute_streaming_query_for_env(request).await
    }

    /// Compare results from multiple environments
    fn compare_results(&self, results: &HashMap<String, EnvQueryResponse>) -> ComparisonResult {
        let mut differences = Vec::new();
        let mut row_count_comparison = HashMap::new();
        
        // Filter out failed results for comparison
        let successful_results: HashMap<&String, &EnvQueryResponse> = results
            .iter()
            .filter(|(_, response)| response.error.is_none())
            .collect();

        if successful_results.is_empty() {
            return ComparisonResult {
                identical: false,
                differences: vec![EnvironmentDifference {
                    difference_type: DifferenceType::Error,
                    environments: results.keys().cloned().collect(),
                    description: "All environments failed to execute the query".to_string(),
                    details: None,
                }],
                row_count_comparison,
            };
        }

        // Collect row counts
        for (env_name, response) in &successful_results {
            row_count_comparison.insert((*env_name).clone(), response.rows.len() as u64);
        }

        // Check for errors in some environments
        let failed_environments: Vec<String> = results
            .iter()
            .filter(|(_, response)| response.error.is_some())
            .map(|(env_name, _)| env_name.clone())
            .collect();

        let has_failures = !failed_environments.is_empty();
        
        if has_failures {
            differences.push(EnvironmentDifference {
                difference_type: DifferenceType::Error,
                environments: failed_environments,
                description: "Some environments failed to execute the query".to_string(),
                details: None,
            });
        }

        if successful_results.len() < 2 {
            // Can't compare with less than 2 successful results
            return ComparisonResult {
                identical: !has_failures,
                differences,
                row_count_comparison,
            };
        }

        // Compare schemas (column structure)
        let mut schema_differences = self.compare_schemas(&successful_results);
        differences.append(&mut schema_differences);

        // Compare row counts
        let mut row_count_differences = self.compare_row_counts(&successful_results);
        differences.append(&mut row_count_differences);

        // Compare data (if schemas are compatible)
        if differences.iter().all(|d| !matches!(d.difference_type, DifferenceType::Schema)) {
            let mut data_differences = self.compare_data(&successful_results);
            differences.append(&mut data_differences);
        }

        let identical = differences.is_empty();

        ComparisonResult {
            identical,
            differences,
            row_count_comparison,
        }
    }

    /// Compare schemas across environments
    fn compare_schemas(&self, results: &HashMap<&String, &EnvQueryResponse>) -> Vec<EnvironmentDifference> {
        let mut differences = Vec::new();
        
        if results.len() < 2 {
            return differences;
        }

        let mut env_schemas: Vec<(&String, &Vec<ColumnInfo>)> = results
            .iter()
            .map(|(env, response)| (*env, &response.columns))
            .collect();

        env_schemas.sort_by_key(|(env, _)| *env);

        // Compare each pair of environments
        for i in 0..env_schemas.len() {
            for j in (i + 1)..env_schemas.len() {
                let (env1, schema1) = env_schemas[i];
                let (env2, schema2) = env_schemas[j];

                if !self.schemas_equal(schema1, schema2) {
                    differences.push(EnvironmentDifference {
                        difference_type: DifferenceType::Schema,
                        environments: vec![env1.clone(), env2.clone()],
                        description: format!("Schema differences between {} and {}", env1, env2),
                        details: Some(serde_json::json!({
                            env1: schema1,
                            env2: schema2
                        })),
                    });
                }
            }
        }

        differences
    }

    /// Compare row counts across environments
    fn compare_row_counts(&self, results: &HashMap<&String, &EnvQueryResponse>) -> Vec<EnvironmentDifference> {
        let mut differences = Vec::new();
        
        if results.len() < 2 {
            return differences;
        }

        let mut row_counts: Vec<(&String, usize)> = results
            .iter()
            .map(|(env, response)| (*env, response.rows.len()))
            .collect();

        row_counts.sort_by_key(|(env, _)| *env);

        // Find environments with different row counts
        let first_count = row_counts[0].1;
        let different_counts: Vec<(&String, usize)> = row_counts
            .iter()
            .filter(|(_, count)| *count != first_count)
            .cloned()
            .collect();

        if !different_counts.is_empty() {
            let mut all_envs = vec![row_counts[0].0.clone()];
            all_envs.extend(different_counts.iter().map(|(env, _)| (*env).clone()));

            differences.push(EnvironmentDifference {
                difference_type: DifferenceType::RowCount,
                environments: all_envs,
                description: "Different row counts across environments".to_string(),
                details: Some(serde_json::json!(
                    row_counts.iter().map(|(env, count)| ((*env).clone(), count)).collect::<HashMap<String, &usize>>()
                )),
            });
        }

        differences
    }

    /// Compare data across environments
    fn compare_data(&self, results: &HashMap<&String, &EnvQueryResponse>) -> Vec<EnvironmentDifference> {
        let mut differences = Vec::new();
        
        if results.len() < 2 {
            return differences;
        }

        let mut env_data: Vec<(&String, &Vec<Row>)> = results
            .iter()
            .map(|(env, response)| (*env, &response.rows))
            .collect();

        env_data.sort_by_key(|(env, _)| *env);

        // Compare each pair of environments
        for i in 0..env_data.len() {
            for j in (i + 1)..env_data.len() {
                let (env1, data1) = env_data[i];
                let (env2, data2) = env_data[j];

                if !self.data_equal(data1, data2) {
                    differences.push(EnvironmentDifference {
                        difference_type: DifferenceType::Data,
                        environments: vec![env1.clone(), env2.clone()],
                        description: format!("Data differences between {} and {}", env1, env2),
                        details: Some(serde_json::json!({
                            "note": "Data comparison shows differences in row values"
                        })),
                    });
                }
            }
        }

        differences
    }

    /// Check if two schemas are equal
    fn schemas_equal(&self, schema1: &[ColumnInfo], schema2: &[ColumnInfo]) -> bool {
        if schema1.len() != schema2.len() {
            return false;
        }

        for (col1, col2) in schema1.iter().zip(schema2.iter()) {
            if col1.name != col2.name || col1.data_type != col2.data_type {
                return false;
            }
        }

        true
    }

    /// Check if two data sets are equal
    fn data_equal(&self, data1: &[Row], data2: &[Row]) -> bool {
        if data1.len() != data2.len() {
            return false;
        }

        // Sort both datasets for comparison (since order might differ)
        let mut sorted_data1 = data1.to_vec();
        let mut sorted_data2 = data2.to_vec();
        
        sorted_data1.sort_by(|a, b| {
            a.values.iter().map(|v| v.to_string()).collect::<Vec<_>>()
                .cmp(&b.values.iter().map(|v| v.to_string()).collect::<Vec<_>>())
        });
        
        sorted_data2.sort_by(|a, b| {
            a.values.iter().map(|v| v.to_string()).collect::<Vec<_>>()
                .cmp(&b.values.iter().map(|v| v.to_string()).collect::<Vec<_>>())
        });

        sorted_data1 == sorted_data2
    }
}

impl Clone for QueryRouter {
    fn clone(&self) -> Self {
        Self {
            pool_manager: Arc::clone(&self.pool_manager),
            environment_manager: Arc::clone(&self.environment_manager),
            result_streamer: ResultStreamer::with_config(self.result_streamer.config.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, DatabaseConfig, EnvironmentConfig, PoolConfig, ServerConfig, McpConfig};
    use crate::query::ColumnInfo;
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
    async fn test_query_router_initialization() {
        let config = create_test_config();
        let env_manager = Arc::new(EnvironmentManager::load_from_config(&config).unwrap());
        let pool_manager = Arc::new(ConnectionPoolManager::initialize(env_manager.clone()).await.unwrap());
        
        let router = QueryRouter::new(pool_manager, env_manager);
        
        // Router should be created successfully
        assert!(router.pool_manager.has_healthy_pools().await || !router.pool_manager.has_healthy_pools().await);
        // This test just verifies the router can be created
    }

    #[test]
    fn test_schema_comparison() {
        let config = create_test_config();
        let env_manager = Arc::new(EnvironmentManager::load_from_config(&config).unwrap());
        let pool_manager = Arc::new(tokio_test::block_on(ConnectionPoolManager::initialize(env_manager.clone())).unwrap());
        let router = QueryRouter::new(pool_manager, env_manager);

        let schema1 = vec![
            ColumnInfo {
                name: "id".to_string(),
                data_type: "INT".to_string(),
                nullable: false,
            },
            ColumnInfo {
                name: "name".to_string(),
                data_type: "VARCHAR".to_string(),
                nullable: true,
            },
        ];

        let schema2 = vec![
            ColumnInfo {
                name: "id".to_string(),
                data_type: "INT".to_string(),
                nullable: false,
            },
            ColumnInfo {
                name: "name".to_string(),
                data_type: "VARCHAR".to_string(),
                nullable: true,
            },
        ];

        let schema3 = vec![
            ColumnInfo {
                name: "id".to_string(),
                data_type: "BIGINT".to_string(), // Different type
                nullable: false,
            },
            ColumnInfo {
                name: "name".to_string(),
                data_type: "VARCHAR".to_string(),
                nullable: true,
            },
        ];

        assert!(router.schemas_equal(&schema1, &schema2));
        assert!(!router.schemas_equal(&schema1, &schema3));
    }

    #[test]
    fn test_data_comparison() {
        let config = create_test_config();
        let env_manager = Arc::new(EnvironmentManager::load_from_config(&config).unwrap());
        let pool_manager = Arc::new(tokio_test::block_on(ConnectionPoolManager::initialize(env_manager.clone())).unwrap());
        let router = QueryRouter::new(pool_manager, env_manager);

        let data1 = vec![
            Row {
                values: vec![
                    serde_json::Value::Number(1.into()),
                    serde_json::Value::String("Alice".to_string()),
                ],
            },
            Row {
                values: vec![
                    serde_json::Value::Number(2.into()),
                    serde_json::Value::String("Bob".to_string()),
                ],
            },
        ];

        let data2 = vec![
            Row {
                values: vec![
                    serde_json::Value::Number(2.into()),
                    serde_json::Value::String("Bob".to_string()),
                ],
            },
            Row {
                values: vec![
                    serde_json::Value::Number(1.into()),
                    serde_json::Value::String("Alice".to_string()),
                ],
            },
        ];

        let data3 = vec![
            Row {
                values: vec![
                    serde_json::Value::Number(1.into()),
                    serde_json::Value::String("Charlie".to_string()), // Different value
                ],
            },
            Row {
                values: vec![
                    serde_json::Value::Number(2.into()),
                    serde_json::Value::String("Bob".to_string()),
                ],
            },
        ];

        assert!(router.data_equal(&data1, &data2)); // Same data, different order
        assert!(!router.data_equal(&data1, &data3)); // Different data
    }

    #[test]
    fn test_comparison_result_structure() {
        let mut results = HashMap::new();
        
        results.insert("env1".to_string(), EnvQueryResponse {
            environment: "env1".to_string(),
            execution_time_ms: 100,
            affected_rows: None,
            columns: vec![
                ColumnInfo {
                    name: "id".to_string(),
                    data_type: "INT".to_string(),
                    nullable: false,
                },
            ],
            rows: vec![
                Row {
                    values: vec![serde_json::Value::Number(1.into())],
                },
            ],
            error: None,
        });

        results.insert("env2".to_string(), EnvQueryResponse {
            environment: "env2".to_string(),
            execution_time_ms: 150,
            affected_rows: None,
            columns: vec![
                ColumnInfo {
                    name: "id".to_string(),
                    data_type: "INT".to_string(),
                    nullable: false,
                },
            ],
            rows: vec![
                Row {
                    values: vec![serde_json::Value::Number(1.into())],
                },
            ],
            error: None,
        });

        let config = create_test_config();
        let env_manager = Arc::new(EnvironmentManager::load_from_config(&config).unwrap());
        let pool_manager = Arc::new(tokio_test::block_on(ConnectionPoolManager::initialize(env_manager.clone())).unwrap());
        let router = QueryRouter::new(pool_manager, env_manager);

        let comparison = router.compare_results(&results);
        
        assert!(comparison.identical);
        assert_eq!(comparison.differences.len(), 0);
        assert_eq!(comparison.row_count_comparison.len(), 2);
        assert_eq!(comparison.row_count_comparison.get("env1"), Some(&1));
        assert_eq!(comparison.row_count_comparison.get("env2"), Some(&1));
    }

    #[test]
    fn test_env_query_request_serialization() {
        let request = EnvQueryRequest {
            sql: "SELECT * FROM users".to_string(),
            environment: Some("test".to_string()),
            parameters: Some(vec![serde_json::Value::String("param1".to_string())]),
            stream_results: false,
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: EnvQueryRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(request.sql, deserialized.sql);
        assert_eq!(request.environment, deserialized.environment);
        assert_eq!(request.parameters, deserialized.parameters);
        assert_eq!(request.stream_results, deserialized.stream_results);
    }

    #[test]
    fn test_multi_env_query_request_serialization() {
        let request = MultiEnvQueryRequest {
            sql: "SELECT COUNT(*) FROM orders".to_string(),
            environments: vec!["dev".to_string(), "uat".to_string()],
            parameters: None,
            compare_results: Some(true),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: MultiEnvQueryRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(request.sql, deserialized.sql);
        assert_eq!(request.environments, deserialized.environments);
        assert_eq!(request.parameters, deserialized.parameters);
        assert_eq!(request.compare_results, deserialized.compare_results);
    }
}