//! Result streaming functionality

use serde::{Serialize, Deserialize};
use crate::query::{Row, QueryResult, ColumnInfo};
use crate::Result;
use sqlx::{MySqlConnection, Row as SqlxRow, Column, TypeInfo};
use tracing::{info, debug, error, warn};
use tokio_stream::{Stream, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;


/// A chunk of streaming results
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ResultChunk {
    /// Unique identifier for this chunk
    pub chunk_id: u64,
    /// Rows in this chunk
    pub rows: Vec<Row>,
    /// Whether this is the final chunk
    pub is_final: bool,
    /// Total number of rows (if known)
    pub total_rows: Option<u64>,
    /// Environment this chunk came from (for multi-environment streaming)
    pub environment: Option<String>,
}

/// A chunk of streaming results from multiple environments
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct MultiEnvResultChunk {
    /// Unique identifier for this chunk
    pub chunk_id: u64,
    /// Results from each environment (keyed by environment name)
    pub environment_chunks: HashMap<String, ResultChunk>,
    /// Whether this is the final chunk for all environments
    pub is_final: bool,
    /// Environments that have completed streaming
    pub completed_environments: Vec<String>,
    /// Environments that encountered errors
    pub failed_environments: HashMap<String, String>,
}

/// Streaming configuration
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Number of rows per chunk
    pub chunk_size: usize,
    /// Maximum number of chunks to buffer
    pub max_buffer_size: usize,
    /// Maximum concurrent streams for multi-environment streaming
    pub max_concurrent_streams: usize,
    /// Timeout for individual environment streams (in seconds)
    pub stream_timeout_seconds: u64,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            chunk_size: 100,
            max_buffer_size: 10,
            max_concurrent_streams: 5,
            stream_timeout_seconds: 300, // 5 minutes
        }
    }
}

/// Result streamer for handling large query results
pub struct ResultStreamer {
    pub config: StreamingConfig,
}

impl ResultStreamer {
    /// Create a new result streamer with default configuration
    pub fn new() -> Self {
        Self {
            config: StreamingConfig::default(),
        }
    }

    /// Create a new result streamer with custom configuration
    pub fn with_config(config: StreamingConfig) -> Self {
        Self { config }
    }

    /// Execute a streaming SELECT query (simplified version)
    pub async fn execute_streaming_query(
        &self,
        connection: &mut MySqlConnection,
        sql: &str,
    ) -> Result<Vec<ResultChunk>> {
        info!("Executing streaming query: {}", sql);
        
        // For now, execute the query normally and then chunk the results
        // This is a simplified implementation that avoids complex lifetime issues
        use sqlx::Executor;
        let rows = connection.fetch_all(sql).await?;
        
        if rows.is_empty() {
            return Ok(vec![ResultChunk {
                chunk_id: 0,
                rows: vec![],
                is_final: true,
                total_rows: Some(0),
                environment: None,
            }]);
        }

        // Convert all rows to our Row format
        let mut converted_rows = Vec::new();
        for row in rows {
            let values = Self::convert_row_to_json_values(&row)?;
            converted_rows.push(Row { values });
        }

        let total_rows = converted_rows.len() as u64;
        let chunk_size = self.config.chunk_size;
        
        // Create chunks
        let mut chunks = Vec::new();
        for (i, chunk_rows) in converted_rows.chunks(chunk_size).enumerate() {
            let is_final = (i + 1) * chunk_size >= converted_rows.len();
            chunks.push(ResultChunk {
                chunk_id: i as u64,
                rows: chunk_rows.to_vec(),
                is_final,
                total_rows: Some(total_rows),
                environment: None,
            });
        }

        Ok(chunks)
    }

    /// Execute a non-streaming query and convert to streaming format
    pub async fn execute_query_as_stream(
        &self,
        connection: &mut MySqlConnection,
        sql: &str,
    ) -> Result<Vec<ResultChunk>> {
        info!("Executing query as stream: {}", sql);
        
        // Execute the query normally first
        use sqlx::Executor;
        let rows = connection.fetch_all(sql).await?;
        
        if rows.is_empty() {
            // Return empty result
            return Ok(vec![ResultChunk {
                chunk_id: 0,
                rows: vec![],
                is_final: true,
                total_rows: Some(0),
                environment: None,
            }]);
        }

        // Extract column information from the first row
        let first_row = &rows[0];
        let _columns = first_row.columns()
            .iter()
            .map(|col| ColumnInfo {
                name: col.name().to_string(),
                data_type: col.type_info().name().to_string(),
                nullable: true,
            })
            .collect::<Vec<_>>();

        // Convert all rows to our Row format
        let mut converted_rows = Vec::new();
        for row in rows {
            let values = Self::convert_row_to_json_values(&row)?;
            converted_rows.push(Row { values });
        }

        let total_rows = converted_rows.len() as u64;
        let chunk_size = self.config.chunk_size;
        
        // Create chunks
        let chunks: Vec<ResultChunk> = converted_rows
            .chunks(chunk_size)
            .enumerate()
            .map(|(i, chunk_rows)| {
                let is_final = (i + 1) * chunk_size >= converted_rows.len();
                ResultChunk {
                    chunk_id: i as u64,
                    rows: chunk_rows.to_vec(),
                    is_final,
                    total_rows: Some(total_rows),
                    environment: None, // Will be set by caller if needed
                }
            })
            .collect();

        Ok(chunks)
    }

    /// Execute streaming queries across multiple environments simultaneously
    /// This method is designed to work with connection pools and proper resource management
    pub async fn execute_multi_env_streaming_query_with_pool<F, Fut>(
        &self,
        environments: Vec<String>,
        sql: &str,
        connection_provider: F,
    ) -> Result<Vec<MultiEnvResultChunk>>
    where
        F: Fn(String) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<sqlx::pool::PoolConnection<sqlx::MySql>>> + Send + 'static,
    {
        info!("Executing multi-environment streaming query across {} environments", environments.len());
        
        if environments.is_empty() {
            return Ok(vec![]);
        }

        // Limit concurrent streams based on configuration
        let max_concurrent = std::cmp::min(environments.len(), self.config.max_concurrent_streams);
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
        
        // Create channels for collecting results from each environment
        let (tx, mut rx) = mpsc::unbounded_channel::<(String, Result<Vec<ResultChunk>>)>();
        
        // Start streaming tasks for each environment with concurrency control
        let mut tasks = Vec::new();
        
        for env_name in environments {
            let env_name_clone = env_name.clone();
            let sql_clone = sql.to_string();
            let tx_clone = tx.clone();
            let connection_provider_clone = connection_provider.clone();
            let semaphore_clone = Arc::clone(&semaphore);
            let timeout_seconds = self.config.stream_timeout_seconds;
            
            let task = tokio::spawn(async move {
                // Acquire semaphore permit for concurrency control
                let _permit = semaphore_clone.acquire().await.map_err(|e| {
                    crate::ServerError::internal_error(
                        format!("Failed to acquire streaming permit for environment '{}'", env_name_clone),
                        Some(e.to_string())
                    )
                });
                
                if _permit.is_err() {
                    let _ = tx_clone.send((env_name_clone.clone(), Err(_permit.unwrap_err())));
                    return;
                }
                
                // Execute streaming query with timeout and proper error handling
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(timeout_seconds),
                    async {
                        match connection_provider_clone(env_name_clone.clone()).await {
                            Ok(mut connection) => {
                                Self::execute_streaming_query_on_connection(
                                    &mut connection,
                                    &sql_clone,
                                    &env_name_clone,
                                ).await
                            }
                            Err(e) => {
                                error!("Failed to get connection for environment '{}': {}", env_name_clone, e);
                                Err(e)
                            }
                        }
                    }
                ).await;
                
                let final_result = match result {
                    Ok(query_result) => query_result,
                    Err(_) => {
                        error!("Streaming query timed out for environment '{}'", env_name_clone);
                        Err(crate::ServerError::internal_error(
                            format!("Streaming query timed out for environment '{}'", env_name_clone),
                            Some(format!("Timeout after {} seconds", timeout_seconds))
                        ))
                    }
                };
                
                let _ = tx_clone.send((env_name_clone, final_result));
            });
            
            tasks.push(task);
        }
        
        // Drop the original sender so the receiver knows when all tasks are done
        drop(tx);
        
        // Collect results from all environments
        let mut env_results: HashMap<String, Result<Vec<ResultChunk>>> = HashMap::new();
        
        while let Some((env_name, result)) = rx.recv().await {
            env_results.insert(env_name, result);
        }
        
        // Wait for all tasks to complete
        for task in tasks {
            if let Err(e) = task.await {
                error!("Streaming task failed: {}", e);
            }
        }
        
        // Merge results into multi-environment chunks
        self.merge_multi_env_results(env_results)
    }

    /// Execute streaming query on a specific connection with proper error handling
    async fn execute_streaming_query_on_connection(
        connection: &mut sqlx::pool::PoolConnection<sqlx::MySql>,
        sql: &str,
        env_name: &str,
    ) -> Result<Vec<ResultChunk>> {
        info!("Executing streaming query for environment '{}'", env_name);
        
        // Execute the query and convert to streaming format
        use sqlx::Executor;
        let rows = connection.fetch_all(sql).await.map_err(|e| {
            error!("Query execution failed for environment '{}': {}", env_name, e);
            crate::ServerError::query_error(sql.to_string(), e)
        })?;
        
        if rows.is_empty() {
            return Ok(vec![ResultChunk {
                chunk_id: 0,
                rows: vec![],
                is_final: true,
                total_rows: Some(0),
                environment: Some(env_name.to_string()),
            }]);
        }

        // Convert all rows to our Row format
        let mut converted_rows = Vec::new();
        for row in rows {
            let values = Self::convert_row_to_json_values(&row)?;
            converted_rows.push(Row { values });
        }

        let total_rows = converted_rows.len() as u64;
        let chunk_size = 100; // Use a reasonable default chunk size
        
        // Create chunks with environment tagging
        let chunks: Vec<ResultChunk> = converted_rows
            .chunks(chunk_size)
            .enumerate()
            .map(|(i, chunk_rows)| {
                let is_final = (i + 1) * chunk_size >= converted_rows.len();
                ResultChunk {
                    chunk_id: i as u64,
                    rows: chunk_rows.to_vec(),
                    is_final,
                    total_rows: Some(total_rows),
                    environment: Some(env_name.to_string()),
                }
            })
            .collect();

        info!("Successfully created {} chunks for environment '{}'", chunks.len(), env_name);
        Ok(chunks)
    }



    /// Merge results from multiple environments into multi-environment chunks
    /// Handles partial failures gracefully and provides detailed error context
    pub fn merge_multi_env_results(
        &self,
        env_results: HashMap<String, Result<Vec<ResultChunk>>>,
    ) -> Result<Vec<MultiEnvResultChunk>> {
        let mut multi_chunks = Vec::new();
        let mut completed_environments = Vec::new();
        let mut failed_environments = HashMap::new();
        
        // Separate successful and failed results with detailed error tracking
        let mut successful_results: HashMap<String, Vec<ResultChunk>> = HashMap::new();
        
        for (env_name, result) in env_results {
            match result {
                Ok(chunks) => {
                    // Validate chunks have proper environment tagging
                    let validated_chunks: Vec<ResultChunk> = chunks
                        .into_iter()
                        .map(|mut chunk| {
                            // Ensure environment is properly tagged
                            if chunk.environment.is_none() {
                                chunk.environment = Some(env_name.clone());
                            }
                            chunk
                        })
                        .collect();
                    
                    successful_results.insert(env_name.clone(), validated_chunks);
                    completed_environments.push(env_name.clone());
                    info!("Successfully processed streaming results for environment '{}'", env_name);
                }
                Err(e) => {
                    error!("Streaming failed for environment '{}': {}", env_name, e);
                    failed_environments.insert(env_name, e.user_message());
                }
            }
        }
        
        // Log summary of results
        info!("Multi-environment streaming summary: {} successful, {} failed", 
              successful_results.len(), failed_environments.len());
        
        if successful_results.is_empty() {
            // All environments failed - return a single chunk with error information
            warn!("All environments failed during multi-environment streaming");
            return Ok(vec![MultiEnvResultChunk {
                chunk_id: 0,
                environment_chunks: HashMap::new(),
                is_final: true,
                completed_environments: Vec::new(),
                failed_environments,
            }]);
        }
        
        // Find the maximum number of chunks across all environments
        let max_chunks = successful_results
            .values()
            .map(|chunks| chunks.len())
            .max()
            .unwrap_or(0);
        
        debug!("Merging {} chunks across {} environments", max_chunks, successful_results.len());
        
        // Create multi-environment chunks by combining chunks at each index
        for chunk_index in 0..max_chunks {
            let mut environment_chunks = HashMap::new();
            let mut is_final_for_all = true;
            let mut environments_in_chunk = Vec::new();
            
            for (env_name, chunks) in &successful_results {
                if let Some(chunk) = chunks.get(chunk_index) {
                    environment_chunks.insert(env_name.clone(), chunk.clone());
                    environments_in_chunk.push(env_name.clone());
                    if !chunk.is_final {
                        is_final_for_all = false;
                    }
                }
            }
            
            let is_final = is_final_for_all && chunk_index == max_chunks - 1;
            
            debug!("Created multi-env chunk {} with {} environments, final: {}", 
                   chunk_index, environments_in_chunk.len(), is_final);
            
            multi_chunks.push(MultiEnvResultChunk {
                chunk_id: chunk_index as u64,
                environment_chunks,
                is_final,
                completed_environments: if is_final { completed_environments.clone() } else { Vec::new() },
                failed_environments: if is_final { failed_environments.clone() } else { HashMap::new() },
            });
        }
        
        if multi_chunks.is_empty() {
            // Create a single empty chunk if no data was found but environments succeeded
            info!("No data found in any environment, creating empty result chunk");
            multi_chunks.push(MultiEnvResultChunk {
                chunk_id: 0,
                environment_chunks: HashMap::new(),
                is_final: true,
                completed_environments,
                failed_environments,
            });
        }
        
        info!("Successfully merged {} multi-environment chunks", multi_chunks.len());
        Ok(multi_chunks)
    }

    /// Execute streaming query with timeout and error handling
    pub async fn execute_streaming_query_with_timeout(
        &self,
        connection: &mut MySqlConnection,
        sql: &str,
        environment: &str,
        timeout_seconds: u64,
    ) -> Result<Vec<ResultChunk>> {
        info!("Executing streaming query with timeout for environment '{}'", environment);
        
        let timeout_duration = std::time::Duration::from_secs(timeout_seconds);
        
        match tokio::time::timeout(timeout_duration, self.execute_streaming_query(connection, sql)).await {
            Ok(result) => {
                // Add environment information to chunks
                match result {
                    Ok(mut chunks) => {
                        for chunk in &mut chunks {
                            chunk.environment = Some(environment.to_string());
                        }
                        info!("Successfully executed streaming query for environment '{}' with {} chunks", 
                              environment, chunks.len());
                        Ok(chunks)
                    }
                    Err(e) => {
                        error!("Streaming query failed for environment '{}': {}", environment, e);
                        Err(e)
                    }
                }
            }
            Err(_) => {
                error!("Streaming query timed out for environment '{}'", environment);
                Err(crate::ServerError::internal_error(
                    format!("Streaming query timed out for environment '{}'", environment),
                    Some(format!("Timeout after {} seconds", timeout_seconds))
                ))
            }
        }
    }

    /// Execute streaming query with advanced error recovery and resource management
    pub async fn execute_streaming_query_with_recovery(
        &self,
        connection: &mut MySqlConnection,
        sql: &str,
        environment: &str,
        max_retries: u32,
    ) -> Result<Vec<ResultChunk>> {
        let mut last_error = None;
        
        for attempt in 0..=max_retries {
            if attempt > 0 {
                let backoff_duration = std::time::Duration::from_millis(100 * (1 << attempt));
                info!("Retrying streaming query for environment '{}' (attempt {}/{}) after {:?}", 
                      environment, attempt + 1, max_retries + 1, backoff_duration);
                tokio::time::sleep(backoff_duration).await;
            }
            
            match self.execute_streaming_query_with_timeout(
                connection, 
                sql, 
                environment, 
                self.config.stream_timeout_seconds
            ).await {
                Ok(chunks) => {
                    if attempt > 0 {
                        info!("Streaming query succeeded for environment '{}' on retry attempt {}", 
                              environment, attempt);
                    }
                    return Ok(chunks);
                }
                Err(e) => {
                    warn!("Streaming query attempt {} failed for environment '{}': {}", 
                          attempt + 1, environment, e);
                    last_error = Some(e);
                }
            }
        }
        
        error!("All streaming query attempts failed for environment '{}'", environment);
        Err(last_error.unwrap_or_else(|| {
            crate::ServerError::internal_error(
                format!("Streaming query failed for environment '{}' after {} attempts", environment, max_retries + 1),
                None
            )
        }))
    }

    /// Get streaming statistics for monitoring and debugging
    pub fn get_streaming_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "config": {
                "chunk_size": self.config.chunk_size,
                "max_buffer_size": self.config.max_buffer_size,
                "max_concurrent_streams": self.config.max_concurrent_streams,
                "stream_timeout_seconds": self.config.stream_timeout_seconds
            },
            "capabilities": {
                "multi_environment_streaming": true,
                "concurrent_streaming": true,
                "error_recovery": true,
                "timeout_handling": true,
                "resource_management": true
            }
        })
    }

    /// Convert a MySQL row to JSON values
    fn convert_row_to_json_values(row: &sqlx::mysql::MySqlRow) -> Result<Vec<serde_json::Value>> {
        let mut values = Vec::new();
        
        for (i, column) in row.columns().iter().enumerate() {
            let value = Self::convert_mysql_value_to_json(row, i, column)?;
            values.push(value);
        }
        
        Ok(values)
    }

    /// Convert a MySQL value to a JSON value
    fn convert_mysql_value_to_json(
        row: &sqlx::mysql::MySqlRow,
        column_index: usize,
        column: &sqlx::mysql::MySqlColumn,
    ) -> Result<serde_json::Value> {
        use sqlx::{Column, TypeInfo, ValueRef};
        
        // Check if the value is NULL first
        if row.try_get_raw(column_index)?.is_null() {
            return Ok(serde_json::Value::Null);
        }

        let type_name = column.type_info().name();
        
        match type_name {
            // Integer types
            "TINYINT" => {
                let val: i8 = row.try_get(column_index)?;
                Ok(serde_json::Value::Number(val.into()))
            }
            "SMALLINT" => {
                let val: i16 = row.try_get(column_index)?;
                Ok(serde_json::Value::Number(val.into()))
            }
            "MEDIUMINT" | "INT" => {
                let val: i32 = row.try_get(column_index)?;
                Ok(serde_json::Value::Number(val.into()))
            }
            "BIGINT" => {
                let val: i64 = row.try_get(column_index)?;
                Ok(serde_json::Value::Number(val.into()))
            }
            
            // Unsigned integer types
            "TINYINT UNSIGNED" => {
                let val: u8 = row.try_get(column_index)?;
                Ok(serde_json::Value::Number(val.into()))
            }
            "SMALLINT UNSIGNED" => {
                let val: u16 = row.try_get(column_index)?;
                Ok(serde_json::Value::Number(val.into()))
            }
            "MEDIUMINT UNSIGNED" | "INT UNSIGNED" => {
                let val: u32 = row.try_get(column_index)?;
                Ok(serde_json::Value::Number(val.into()))
            }
            "BIGINT UNSIGNED" => {
                let val: u64 = row.try_get(column_index)?;
                // JSON numbers are limited to i64 range, so convert large u64 to string
                if val > i64::MAX as u64 {
                    Ok(serde_json::Value::String(val.to_string()))
                } else {
                    Ok(serde_json::Value::Number((val as i64).into()))
                }
            }
            
            // Floating point types
            "FLOAT" => {
                let val: f32 = row.try_get(column_index)?;
                if let Some(num) = serde_json::Number::from_f64(val as f64) {
                    Ok(serde_json::Value::Number(num))
                } else {
                    Ok(serde_json::Value::String(val.to_string()))
                }
            }
            "DOUBLE" => {
                let val: f64 = row.try_get(column_index)?;
                if let Some(num) = serde_json::Number::from_f64(val) {
                    Ok(serde_json::Value::Number(num))
                } else {
                    Ok(serde_json::Value::String(val.to_string()))
                }
            }
            
            // String types
            "CHAR" | "VARCHAR" | "TEXT" | "TINYTEXT" | "MEDIUMTEXT" | "LONGTEXT" => {
                let val: String = row.try_get(column_index)?;
                Ok(serde_json::Value::String(val))
            }
            
            // Binary types - convert to base64 string
            "BINARY" | "VARBINARY" | "BLOB" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" => {
                let val: Vec<u8> = row.try_get(column_index)?;
                use base64::{Engine as _, engine::general_purpose};
                let base64_str = general_purpose::STANDARD.encode(&val);
                Ok(serde_json::Value::String(base64_str))
            }
            
            // Date and time types
            "DATE" => {
                let val: sqlx::types::chrono::NaiveDate = row.try_get(column_index)?;
                Ok(serde_json::Value::String(val.to_string()))
            }
            "TIME" => {
                let val: sqlx::types::chrono::NaiveTime = row.try_get(column_index)?;
                Ok(serde_json::Value::String(val.to_string()))
            }
            "DATETIME" | "TIMESTAMP" => {
                let val: sqlx::types::chrono::NaiveDateTime = row.try_get(column_index)?;
                Ok(serde_json::Value::String(val.to_string()))
            }
            
            // Boolean type
            "BOOLEAN" | "BOOL" => {
                let val: bool = row.try_get(column_index)?;
                Ok(serde_json::Value::Bool(val))
            }
            
            // JSON type
            "JSON" => {
                let val: serde_json::Value = row.try_get(column_index)?;
                Ok(val)
            }
            
            // Default case - try to get as string
            _ => {
                debug!("Unknown MySQL type '{}', attempting string conversion", type_name);
                match row.try_get::<String, _>(column_index) {
                    Ok(val) => Ok(serde_json::Value::String(val)),
                    Err(_) => {
                        error!("Failed to convert MySQL type '{}' to JSON", type_name);
                        Ok(serde_json::Value::Null)
                    }
                }
            }
        }
    }

    /// Collect all chunks from a stream into a single QueryResult
    pub async fn collect_stream_to_result(
        mut stream: impl Stream<Item = Result<ResultChunk>> + Unpin,
        execution_time_ms: u64,
    ) -> Result<QueryResult> {
        let mut all_rows = Vec::new();
        let mut columns = Vec::new();
        let mut total_rows = None;
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            
            // Extract columns from first chunk (if we haven't already)
            if columns.is_empty() && !chunk.rows.is_empty() {
                // We need to reconstruct column info - this is a limitation
                // In a real implementation, we'd want to pass column info with the first chunk
                let first_row = &chunk.rows[0];
                for (i, _value) in first_row.values.iter().enumerate() {
                    columns.push(ColumnInfo {
                        name: format!("column_{}", i),
                        data_type: "UNKNOWN".to_string(),
                        nullable: true,
                    });
                }
            }
            
            all_rows.extend(chunk.rows);
            
            if chunk.is_final {
                total_rows = chunk.total_rows;
                break;
            }
        }
        
        Ok(QueryResult {
            columns,
            rows: all_rows,
            affected_rows: total_rows,
            execution_time_ms,
        })
    }
}

impl Default for ResultStreamer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json;
    use crate::query::Row;

    // Import the row generator from query module
    prop_compose! {
        fn arb_row()(
            values in prop::collection::vec(
                prop_oneof![
                    Just(serde_json::Value::Null),
                    any::<bool>().prop_map(serde_json::Value::Bool),
                    any::<i64>().prop_map(|n| serde_json::Value::Number(n.into())),
                    // Use integers to avoid precision issues in JSON round-trip
                    (-1000000i64..1000000i64)
                        .prop_map(|i| serde_json::Value::Number(i.into())),
                    "[\\PC]*".prop_map(serde_json::Value::String),
                ],
                0..10
            )
        ) -> Row {
            Row { values }
        }
    }

    prop_compose! {
        fn arb_result_chunk()(
            chunk_id in any::<u64>(),
            rows in prop::collection::vec(arb_row(), 0..50),
            is_final in any::<bool>(),
            total_rows in prop::option::of(any::<u64>())
        ) -> ResultChunk {
            ResultChunk {
                chunk_id,
                rows,
                is_final,
                total_rows,
                environment: None,
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        /// **Feature: mysql-mcp-server, Property 15: Serialization round-trip**
        /// **Validates: Requirements 4.4**
        #[test]
        fn test_result_chunk_serialization_round_trip(result_chunk in arb_result_chunk()) {
            // Serialize the ResultChunk to JSON
            let serialized = serde_json::to_string(&result_chunk)
                .expect("ResultChunk should serialize to JSON");
            
            // Deserialize back to ResultChunk
            let deserialized: ResultChunk = serde_json::from_str(&serialized)
                .expect("Serialized JSON should deserialize back to ResultChunk");
            
            // Verify round-trip equivalence
            prop_assert_eq!(result_chunk, deserialized);
        }
    }

    #[tokio::test]
    async fn test_multi_environment_streaming_merge() {
        let streamer = ResultStreamer::new();
        let mut env_results = HashMap::new();
        
        // Environment 1 - successful result
        let env1_chunks = vec![
            ResultChunk {
                chunk_id: 0,
                rows: vec![
                    Row {
                        values: vec![
                            serde_json::Value::Number(1.into()),
                            serde_json::Value::String("Data from env1".to_string()),
                        ],
                    },
                ],
                is_final: true,
                total_rows: Some(1),
                environment: Some("env1".to_string()),
            }
        ];
        env_results.insert("env1".to_string(), Ok(env1_chunks));

        // Environment 2 - failed result
        env_results.insert("env2".to_string(), Err(crate::ServerError::internal_error(
            "Connection failed".to_string(),
            None
        )));

        // Merge the results
        let merged_chunks = streamer.merge_multi_env_results(env_results).unwrap();

        assert_eq!(merged_chunks.len(), 1);
        let chunk = &merged_chunks[0];
        assert_eq!(chunk.environment_chunks.len(), 1);
        assert!(chunk.environment_chunks.contains_key("env1"));
        assert_eq!(chunk.failed_environments.len(), 1);
        assert!(chunk.failed_environments.contains_key("env2"));
        assert!(chunk.is_final);
        
        // Verify environment tagging is preserved
        let env1_chunk = chunk.environment_chunks.get("env1").unwrap();
        assert_eq!(env1_chunk.environment, Some("env1".to_string()));
    }

    #[tokio::test]
    async fn test_streaming_stats() {
        let streamer = ResultStreamer::new();
        let stats = streamer.get_streaming_stats();
        
        assert!(stats.get("config").is_some());
        assert!(stats.get("capabilities").is_some());
        
        let config = stats.get("config").unwrap();
        assert_eq!(config.get("chunk_size").unwrap().as_u64().unwrap(), 100);
        assert_eq!(config.get("max_concurrent_streams").unwrap().as_u64().unwrap(), 5);
        
        let capabilities = stats.get("capabilities").unwrap();
        assert_eq!(capabilities.get("multi_environment_streaming").unwrap().as_bool().unwrap(), true);
        assert_eq!(capabilities.get("concurrent_streaming").unwrap().as_bool().unwrap(), true);
    }

    #[tokio::test]
    async fn test_multi_environment_streaming_with_multiple_chunks() {
        let streamer = ResultStreamer::new();
        let mut env_results = HashMap::new();
        
        // Environment 1 - multiple chunks
        let env1_chunks = vec![
            ResultChunk {
                chunk_id: 0,
                rows: vec![
                    Row { values: vec![serde_json::Value::Number(1.into())] },
                    Row { values: vec![serde_json::Value::Number(2.into())] },
                ],
                is_final: false,
                total_rows: Some(4),
                environment: Some("env1".to_string()),
            },
            ResultChunk {
                chunk_id: 1,
                rows: vec![
                    Row { values: vec![serde_json::Value::Number(3.into())] },
                    Row { values: vec![serde_json::Value::Number(4.into())] },
                ],
                is_final: true,
                total_rows: Some(4),
                environment: Some("env1".to_string()),
            }
        ];
        env_results.insert("env1".to_string(), Ok(env1_chunks));

        // Environment 2 - single chunk
        let env2_chunks = vec![
            ResultChunk {
                chunk_id: 0,
                rows: vec![
                    Row { values: vec![serde_json::Value::String("data".to_string())] },
                ],
                is_final: true,
                total_rows: Some(1),
                environment: Some("env2".to_string()),
            }
        ];
        env_results.insert("env2".to_string(), Ok(env2_chunks));

        // Merge the results
        let merged_chunks = streamer.merge_multi_env_results(env_results).unwrap();

        // Should have 2 chunks (max across environments)
        assert_eq!(merged_chunks.len(), 2);
        
        // First chunk should have data from both environments
        let first_chunk = &merged_chunks[0];
        assert_eq!(first_chunk.chunk_id, 0);
        assert!(!first_chunk.is_final);
        assert_eq!(first_chunk.environment_chunks.len(), 2);
        assert!(first_chunk.environment_chunks.contains_key("env1"));
        assert!(first_chunk.environment_chunks.contains_key("env2"));
        
        // Second chunk should only have data from env1 (env2 finished)
        let second_chunk = &merged_chunks[1];
        assert_eq!(second_chunk.chunk_id, 1);
        assert!(second_chunk.is_final);
        assert_eq!(second_chunk.environment_chunks.len(), 1);
        assert!(second_chunk.environment_chunks.contains_key("env1"));
        assert!(!second_chunk.environment_chunks.contains_key("env2"));
    }

    #[tokio::test]
    async fn test_streaming_config_defaults() {
        let config = StreamingConfig::default();
        assert_eq!(config.chunk_size, 100);
        assert_eq!(config.max_buffer_size, 10);
        assert_eq!(config.max_concurrent_streams, 5);
        assert_eq!(config.stream_timeout_seconds, 300);
    }

    #[tokio::test]
    async fn test_multi_env_result_chunk_serialization() {
        let mut environment_chunks = HashMap::new();
        environment_chunks.insert("env1".to_string(), ResultChunk {
            chunk_id: 0,
            rows: vec![],
            is_final: true,
            total_rows: Some(0),
            environment: Some("env1".to_string()),
        });

        let multi_chunk = MultiEnvResultChunk {
            chunk_id: 0,
            environment_chunks,
            is_final: true,
            completed_environments: vec!["env1".to_string()],
            failed_environments: HashMap::new(),
        };

        // Test serialization
        let serialized = serde_json::to_string(&multi_chunk).unwrap();
        let deserialized: MultiEnvResultChunk = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(multi_chunk.chunk_id, deserialized.chunk_id);
        assert_eq!(multi_chunk.is_final, deserialized.is_final);
        assert_eq!(multi_chunk.completed_environments, deserialized.completed_environments);
        assert_eq!(multi_chunk.failed_environments, deserialized.failed_environments);
    }
}