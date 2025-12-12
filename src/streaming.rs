//! Result streaming functionality

use serde::{Serialize, Deserialize};
use crate::query::{Row, QueryResult, ColumnInfo};
use crate::Result;
use sqlx::{MySqlConnection, Row as SqlxRow, Column, TypeInfo};
use tracing::{info, debug, error};
use tokio_stream::{Stream, StreamExt};


/// A chunk of streaming results
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ResultChunk {
    /// Unique identifier for this chunk
    pub chunk_id: u64,
    /// Rows in this chunk
    pub rows: Vec<Row>,
    /// Whether this is the final chunk
    pub is_final: bool,
    /// Total number of rows (if known)
    pub total_rows: Option<u64>,
}

/// Streaming configuration
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Number of rows per chunk
    pub chunk_size: usize,
    /// Maximum number of chunks to buffer
    pub max_buffer_size: usize,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            chunk_size: 100,
            max_buffer_size: 10,
        }
    }
}

/// Result streamer for handling large query results
pub struct ResultStreamer {
    config: StreamingConfig,
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
                }
            })
            .collect();

        Ok(chunks)
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
}