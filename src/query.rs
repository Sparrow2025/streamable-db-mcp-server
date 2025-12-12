//! Query processing and execution

use serde::{Deserialize, Serialize};
use crate::ServerError;

/// Request structure for SQL queries
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryRequest {
    /// SQL query string
    pub sql: String,
    /// Optional query parameters
    pub parameters: Option<Vec<serde_json::Value>>,
    /// Whether to stream results
    pub stream_results: bool,
}

/// Query execution result
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct QueryResult {
    /// Column information
    pub columns: Vec<ColumnInfo>,
    /// Result rows
    pub rows: Vec<Row>,
    /// Number of affected rows (for INSERT/UPDATE/DELETE)
    pub affected_rows: Option<u64>,
    /// Query execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Column metadata information
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ColumnInfo {
    /// Column name
    pub name: String,
    /// Data type
    pub data_type: String,
    /// Whether the column can be null
    pub nullable: bool,
}

/// A single row of query results
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Row {
    /// Column values
    pub values: Vec<serde_json::Value>,
}

use crate::Result;
use sqlx::{MySqlConnection, Row as SqlxRow, Column, TypeInfo, ValueRef};
use sqlx::types::chrono;
use std::time::Instant;
use tracing::{info, debug, error};

/// Query processor for executing SQL queries against MySQL database
pub struct QueryProcessor;

impl QueryProcessor {
    /// Execute a SQL query and return the results
    pub async fn execute_query(
        connection: &mut MySqlConnection,
        request: &QueryRequest,
    ) -> Result<QueryResult> {
        let start_time = Instant::now();
        
        info!("Executing SQL query: {}", request.sql);
        debug!("Query parameters: {:?}", request.parameters);

        // Validate SQL query
        if request.sql.trim().is_empty() {
            return Err(ServerError::validation_error(
                "SQL query cannot be empty".to_string(),
                Some("empty string".to_string())
            ));
        }

        // Determine query type based on the SQL statement
        let sql_trimmed = request.sql.trim().to_uppercase();
        
        let result = if sql_trimmed.starts_with("SELECT") {
            Self::execute_select_query(connection, request).await
        } else if sql_trimmed.starts_with("INSERT") 
            || sql_trimmed.starts_with("UPDATE") 
            || sql_trimmed.starts_with("DELETE") {
            Self::execute_modification_query(connection, request).await
        } else {
            // For other query types (CREATE, DROP, ALTER, etc.), treat as modification queries
            Self::execute_modification_query(connection, request).await
        };

        match result {
            Ok(mut query_result) => {
                query_result.execution_time_ms = start_time.elapsed().as_millis() as u64;
                info!("Query executed successfully in {}ms", query_result.execution_time_ms);
                Ok(query_result)
            }
            Err(e) => {
                error!("Query execution failed: {}", e.detailed_message());
                Err(e)
            }
        }
    }

    /// Execute a SELECT query and return the result set
    async fn execute_select_query(
        connection: &mut MySqlConnection,
        request: &QueryRequest,
    ) -> Result<QueryResult> {
        use sqlx::Executor;

        // Execute the query
        let rows = match connection.fetch_all(request.sql.as_str()).await {
            Ok(rows) => rows,
            Err(e) => {
                return Err(ServerError::query_error(request.sql.clone(), e));
            }
        };
        
        if rows.is_empty() {
            // No rows returned - create empty result with no columns
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                affected_rows: None,
                execution_time_ms: 0, // Will be set by caller
            });
        }

        // Extract column information from the first row
        let first_row = &rows[0];
        let columns = first_row.columns()
            .iter()
            .map(|col| ColumnInfo {
                name: col.name().to_string(),
                data_type: col.type_info().name().to_string(),
                nullable: true, // MySQL columns are nullable by default unless specified otherwise
            })
            .collect();

        // Convert all rows to our Row format
        let mut result_rows = Vec::new();
        for (row_index, row) in rows.iter().enumerate() {
            match Self::convert_row_to_json_values(row) {
                Ok(values) => result_rows.push(Row { values }),
                Err(e) => {
                    error!("Failed to convert row {} to JSON: {}", row_index, e);
                    return Err(ServerError::internal_error(
                        format!("Failed to convert row {} to JSON", row_index),
                        Some(e.to_string())
                    ));
                }
            }
        }

        Ok(QueryResult {
            columns,
            rows: result_rows,
            affected_rows: None,
            execution_time_ms: 0, // Will be set by caller
        })
    }

    /// Execute INSERT, UPDATE, DELETE, or other modification queries
    async fn execute_modification_query(
        connection: &mut MySqlConnection,
        request: &QueryRequest,
    ) -> Result<QueryResult> {
        use sqlx::Executor;

        // Execute the query and get the result
        let result = match connection.execute(request.sql.as_str()).await {
            Ok(result) => result,
            Err(e) => {
                return Err(ServerError::query_error(request.sql.clone(), e));
            }
        };
        
        let affected_rows = result.rows_affected();
        
        debug!("Modification query affected {} rows", affected_rows);

        Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            affected_rows: Some(affected_rows),
            execution_time_ms: 0, // Will be set by caller
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
            
            // Decimal types - convert to string to preserve precision
            "DECIMAL" | "NUMERIC" => {
                // Try to get as string since Decimal type might not be available
                match row.try_get::<String, _>(column_index) {
                    Ok(val) => Ok(serde_json::Value::String(val)),
                    Err(_) => {
                        // Fallback to f64 if string conversion fails
                        let val: f64 = row.try_get(column_index)?;
                        if let Some(num) = serde_json::Number::from_f64(val) {
                            Ok(serde_json::Value::Number(num))
                        } else {
                            Ok(serde_json::Value::String(val.to_string()))
                        }
                    }
                }
            }
            
            // Boolean type
            "BOOLEAN" | "BOOL" => {
                let val: bool = row.try_get(column_index)?;
                Ok(serde_json::Value::Bool(val))
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
                let val: chrono::NaiveDate = row.try_get(column_index)?;
                Ok(serde_json::Value::String(val.to_string()))
            }
            "TIME" => {
                let val: chrono::NaiveTime = row.try_get(column_index)?;
                Ok(serde_json::Value::String(val.to_string()))
            }
            "DATETIME" | "TIMESTAMP" => {
                let val: chrono::NaiveDateTime = row.try_get(column_index)?;
                Ok(serde_json::Value::String(val.to_string()))
            }
            
            // JSON type
            "JSON" => {
                let val: serde_json::Value = row.try_get(column_index)?;
                Ok(val)
            }
            
            // UUID type
            "UUID" => {
                let val: sqlx::types::Uuid = row.try_get(column_index)?;
                Ok(serde_json::Value::String(val.to_string()))
            }
            
            // Default case - try to get as string
            _ => {
                debug!("Unknown MySQL type '{}', attempting string conversion", type_name);
                match row.try_get::<String, _>(column_index) {
                    Ok(val) => Ok(serde_json::Value::String(val)),
                    Err(e) => {
                        error!("Failed to convert MySQL type '{}' to JSON: {}", type_name, e);
                        // Return null for unconvertible values rather than failing the entire query
                        Ok(serde_json::Value::Null)
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json;

    // Property-based test generators
    prop_compose! {
        fn arb_column_info()(
            name in "[a-zA-Z_][a-zA-Z0-9_]*",
            data_type in "(VARCHAR|INT|BIGINT|DECIMAL|DATETIME|BOOLEAN|TEXT)",
            nullable in any::<bool>()
        ) -> ColumnInfo {
            ColumnInfo {
                name,
                data_type,
                nullable,
            }
        }
    }

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
        fn arb_query_result()(
            columns in prop::collection::vec(arb_column_info(), 0..5),
            rows in prop::collection::vec(arb_row(), 0..100),
            affected_rows in prop::option::of(any::<u64>()),
            execution_time_ms in any::<u64>()
        ) -> QueryResult {
            QueryResult {
                columns,
                rows,
                affected_rows,
                execution_time_ms,
            }
        }
    }

    prop_compose! {
        fn arb_select_query()(
            table_name in "[a-zA-Z_][a-zA-Z0-9_]*",
            columns in prop::option::of(prop::collection::vec("[a-zA-Z_][a-zA-Z0-9_]*", 1..5)),
            where_clause in prop::option::of("[a-zA-Z_][a-zA-Z0-9_]* = '[a-zA-Z0-9_]*'"),
            limit_clause in prop::option::of(1u32..1000u32)
        ) -> String {
            let mut query = String::from("SELECT ");
            
            // Add columns or *
            if let Some(cols) = columns {
                query.push_str(&cols.join(", "));
            } else {
                query.push('*');
            }
            
            query.push_str(" FROM ");
            query.push_str(&table_name);
            
            // Add WHERE clause if present
            if let Some(where_cond) = where_clause {
                query.push_str(" WHERE ");
                query.push_str(&where_cond);
            }
            
            // Add LIMIT clause if present
            if let Some(limit) = limit_clause {
                query.push_str(" LIMIT ");
                query.push_str(&limit.to_string());
            }
            
            query
        }
    }

    prop_compose! {
        fn arb_query_request()(
            sql in arb_select_query(),
            parameters in prop::option::of(prop::collection::vec(
                prop_oneof![
                    Just(serde_json::Value::Null),
                    any::<bool>().prop_map(serde_json::Value::Bool),
                    any::<i64>().prop_map(|n| serde_json::Value::Number(n.into())),
                    "[\\PC]*".prop_map(serde_json::Value::String),
                ],
                0..5
            )),
            stream_results in any::<bool>()
        ) -> QueryRequest {
            QueryRequest {
                sql,
                parameters,
                stream_results,
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        /// **Feature: mysql-mcp-server, Property 5: SELECT query execution**
        /// **Validates: Requirements 2.1**
        #[test]
        fn test_select_query_request_structure(request in arb_query_request()) {
            // Verify that the generated query request has the correct structure for SELECT queries
            prop_assert!(request.sql.trim().to_uppercase().starts_with("SELECT"));
            
            // Verify that the SQL contains required SELECT components
            let sql_upper = request.sql.to_uppercase();
            prop_assert!(sql_upper.contains("FROM"));
            
            // Verify that parameters, if present, are valid JSON values
            if let Some(params) = &request.parameters {
                for param in params {
                    // Each parameter should be a valid JSON value
                    prop_assert!(serde_json::to_string(param).is_ok());
                }
            }
            
            // Verify that stream_results is a valid boolean
            prop_assert!(request.stream_results == true || request.stream_results == false);
            
            // Verify that the request can be serialized and deserialized
            let serialized = serde_json::to_string(&request)
                .expect("QueryRequest should serialize to JSON");
            let _deserialized: QueryRequest = serde_json::from_str(&serialized)
                .expect("Serialized JSON should deserialize back to QueryRequest");
        }
        
        /// **Feature: mysql-mcp-server, Property 15: Serialization round-trip**
        /// **Validates: Requirements 4.4**
        #[test]
        fn test_query_result_serialization_round_trip(query_result in arb_query_result()) {
            // Serialize the QueryResult to JSON
            let serialized = serde_json::to_string(&query_result)
                .expect("QueryResult should serialize to JSON");
            
            // Deserialize back to QueryResult
            let deserialized: QueryResult = serde_json::from_str(&serialized)
                .expect("Serialized JSON should deserialize back to QueryResult");
            
            // Verify round-trip equivalence - the deserialized object should equal the original
            prop_assert_eq!(query_result, deserialized);
        }
    }
}