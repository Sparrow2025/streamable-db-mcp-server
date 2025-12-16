//! Enhanced MCP Tools for multi-environment support
//! 
//! This module provides environment-aware versions of existing MCP tools and new
//! multi-environment specific tools for querying and comparing data across environments.

use crate::{Result, ServerError};
use crate::environment::EnvironmentManager;
use crate::pool::ConnectionPoolManager;
use crate::router::{QueryRouter, EnvQueryRequest, MultiEnvQueryRequest};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, debug, error};

/// Enhanced MCP Tools handler for multi-environment operations
pub struct EnhancedMcpTools {
    /// Query router for environment-aware query execution
    query_router: Arc<QueryRouter>,
    /// Environment manager for environment metadata
    environment_manager: Arc<EnvironmentManager>,
    /// Connection pool manager for direct connection access
    pool_manager: Arc<ConnectionPoolManager>,
}

/// Schema comparison result between environments
#[derive(Debug, Serialize, Deserialize)]
pub struct SchemaComparisonResult {
    /// Environments being compared
    pub environments: Vec<String>,
    /// Whether schemas are identical
    pub identical: bool,
    /// Schema differences found
    pub differences: Vec<SchemaDifference>,
    /// Detailed schema information per environment
    pub schemas: HashMap<String, Vec<TableSchema>>,
}

/// Schema difference between environments
#[derive(Debug, Serialize, Deserialize)]
pub struct SchemaDifference {
    /// Type of difference
    pub difference_type: SchemaDifferenceType,
    /// Table or object name
    pub object_name: String,
    /// Environments involved
    pub environments: Vec<String>,
    /// Description of the difference
    pub description: String,
    /// Additional details
    pub details: Option<Value>,
}

/// Types of schema differences
#[derive(Debug, Serialize, Deserialize)]
pub enum SchemaDifferenceType {
    /// Table exists in some environments but not others
    TableExistence,
    /// Column differences in a table
    ColumnDifference,
    /// Index differences
    IndexDifference,
    /// Constraint differences
    ConstraintDifference,
}

/// Table schema information
#[derive(Debug, Serialize, Deserialize)]
pub struct TableSchema {
    /// Table name
    pub table_name: String,
    /// Database name
    pub database_name: String,
    /// Column information
    pub columns: Vec<ColumnSchema>,
    /// Index information
    pub indexes: Vec<IndexSchema>,
    /// Constraint information
    pub constraints: Vec<ConstraintSchema>,
}

/// Column schema information
#[derive(Debug, Serialize, Deserialize)]
pub struct ColumnSchema {
    /// Column name
    pub name: String,
    /// Data type
    pub data_type: String,
    /// Whether column is nullable
    pub nullable: bool,
    /// Default value
    pub default_value: Option<String>,
    /// Whether column is part of primary key
    pub is_primary_key: bool,
    /// Whether column is auto increment
    pub is_auto_increment: bool,
    /// Column comment
    pub comment: Option<String>,
}

/// Index schema information
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexSchema {
    /// Index name
    pub name: String,
    /// Whether index is unique
    pub is_unique: bool,
    /// Columns in the index
    pub columns: Vec<String>,
    /// Index type
    pub index_type: String,
}

/// Constraint schema information
#[derive(Debug, Serialize, Deserialize)]
pub struct ConstraintSchema {
    /// Constraint name
    pub name: String,
    /// Constraint type (PRIMARY KEY, FOREIGN KEY, UNIQUE, CHECK)
    pub constraint_type: String,
    /// Columns involved in constraint
    pub columns: Vec<String>,
    /// Referenced table (for foreign keys)
    pub referenced_table: Option<String>,
    /// Referenced columns (for foreign keys)
    pub referenced_columns: Option<Vec<String>>,
}

impl EnhancedMcpTools {
    /// Create a new EnhancedMcpTools instance
    pub fn new(
        query_router: Arc<QueryRouter>,
        environment_manager: Arc<EnvironmentManager>,
        pool_manager: Arc<ConnectionPoolManager>,
    ) -> Self {
        Self {
            query_router,
            environment_manager,
            pool_manager,
        }
    }

    /// Get list of available enhanced MCP tools
    pub fn get_tool_definitions(&self) -> Value {
        json!({
            "tools": [
                {
                    "name": "execute_query_env",
                    "description": "Execute read-only SQL queries against a specific environment. Supports environment selection and maintains all security restrictions.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "sql": {
                                "type": "string",
                                "description": "Read-only SQL query to execute (SELECT, SHOW, DESCRIBE, EXPLAIN only)"
                            },
                            "environment": {
                                "type": "string",
                                "description": "Target environment name (optional, uses default if not specified)"
                            },
                            "parameters": {
                                "type": "array",
                                "description": "Optional query parameters",
                                "items": {}
                            },
                            "stream_results": {
                                "type": "boolean",
                                "description": "Whether to stream results for large datasets",
                                "default": false
                            }
                        },
                        "required": ["sql"]
                    }
                },
                {
                    "name": "execute_query_multi_env",
                    "description": "Execute the same read-only SQL query against multiple environments simultaneously. Useful for comparing data across environments.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "sql": {
                                "type": "string",
                                "description": "Read-only SQL query to execute (SELECT, SHOW, DESCRIBE, EXPLAIN only)"
                            },
                            "environments": {
                                "type": "array",
                                "description": "List of target environments (if empty, uses all enabled environments)",
                                "items": {
                                    "type": "string"
                                }
                            },
                            "parameters": {
                                "type": "array",
                                "description": "Optional query parameters",
                                "items": {}
                            },
                            "compare_results": {
                                "type": "boolean",
                                "description": "Whether to compare results across environments",
                                "default": false
                            }
                        },
                        "required": ["sql"]
                    }
                },
                {
                    "name": "list_environments",
                    "description": "List all configured database environments with their status and connection information.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "include_disabled": {
                                "type": "boolean",
                                "description": "Whether to include disabled environments",
                                "default": false
                            }
                        }
                    }
                },
                {
                    "name": "list_databases_env",
                    "description": "List all databases in a specific environment.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "environment": {
                                "type": "string",
                                "description": "Target environment name (optional, uses default if not specified)"
                            }
                        }
                    }
                },
                {
                    "name": "list_databases_all_env",
                    "description": "List all databases across all enabled environments.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "environments": {
                                "type": "array",
                                "description": "List of environments to query (if empty, uses all enabled environments)",
                                "items": {
                                    "type": "string"
                                }
                            }
                        }
                    }
                },
                {
                    "name": "list_tables_env",
                    "description": "List all tables in a specific database and environment.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "environment": {
                                "type": "string",
                                "description": "Target environment name (optional, uses default if not specified)"
                            },
                            "database": {
                                "type": "string",
                                "description": "Database name (optional, uses current database if not specified)"
                            }
                        }
                    }
                },
                {
                    "name": "describe_table_env",
                    "description": "Get detailed information about a table structure in a specific environment.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "table": {
                                "type": "string",
                                "description": "Table name"
                            },
                            "environment": {
                                "type": "string",
                                "description": "Target environment name (optional, uses default if not specified)"
                            },
                            "database": {
                                "type": "string",
                                "description": "Database name (optional, uses current database if not specified)"
                            }
                        },
                        "required": ["table"]
                    }
                },
                {
                    "name": "compare_schema",
                    "description": "Compare database schema (tables, columns, indexes) across multiple environments.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "environments": {
                                "type": "array",
                                "description": "List of environments to compare (minimum 2 required)",
                                "items": {
                                    "type": "string"
                                },
                                "minItems": 2
                            },
                            "database": {
                                "type": "string",
                                "description": "Database name to compare (optional, uses current database if not specified)"
                            },
                            "table": {
                                "type": "string",
                                "description": "Specific table to compare (optional, compares all tables if not specified)"
                            }
                        },
                        "required": ["environments"]
                    }
                },
                {
                    "name": "health_check_env",
                    "description": "Check the health status of database connections for specific environments with optional comprehensive diagnostics.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "environment": {
                                "type": "string",
                                "description": "Specific environment to check (optional, checks all environments if not specified)"
                            },
                            "comprehensive": {
                                "type": "boolean",
                                "description": "Whether to perform comprehensive health checks with detailed diagnostics",
                                "default": false
                            }
                        }
                    }
                },
                {
                    "name": "get_monitoring_report",
                    "description": "Get comprehensive monitoring report for all environments including statistics and performance metrics.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "get_performance_metrics",
                    "description": "Get detailed performance metrics for specific environments including query statistics and connection metrics.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "environment": {
                                "type": "string",
                                "description": "Specific environment to get metrics for (optional, gets metrics for all environments if not specified)"
                            }
                        }
                    }
                },
                {
                    "name": "test_connection_env",
                    "description": "Test database connection for a specific environment with detailed diagnostics.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "environment": {
                                "type": "string",
                                "description": "Environment name to test"
                            }
                        },
                        "required": ["environment"]
                    }
                },
                {
                    "name": "execute_streaming_query_env",
                    "description": "Execute a streaming read-only SQL query against a specific environment. Returns results in chunks for large datasets.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "sql": {
                                "type": "string",
                                "description": "Read-only SQL query to execute (SELECT, SHOW, DESCRIBE, EXPLAIN only)"
                            },
                            "environment": {
                                "type": "string",
                                "description": "Target environment name (optional, uses default if not specified)"
                            },
                            "parameters": {
                                "type": "array",
                                "description": "Optional query parameters",
                                "items": {}
                            }
                        },
                        "required": ["sql"]
                    }
                },
                {
                    "name": "execute_streaming_query_multi_env",
                    "description": "Execute a streaming read-only SQL query against multiple environments simultaneously. Returns chunked results from all environments.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "sql": {
                                "type": "string",
                                "description": "Read-only SQL query to execute (SELECT, SHOW, DESCRIBE, EXPLAIN only)"
                            },
                            "environments": {
                                "type": "array",
                                "description": "List of target environments (if empty, uses all enabled environments)",
                                "items": {
                                    "type": "string"
                                }
                            },
                            "parameters": {
                                "type": "array",
                                "description": "Optional query parameters",
                                "items": {}
                            }
                        },
                        "required": ["sql"]
                    }
                }
            ]
        })
    }

    /// Handle enhanced MCP tool calls
    pub async fn handle_tool_call(&self, tool_name: &str, arguments: Value) -> Result<Value> {
        debug!("Handling enhanced MCP tool call: {} with arguments: {}", tool_name, arguments);

        match tool_name {
            "execute_query_env" => self.handle_execute_query_env(arguments).await,
            "execute_query_multi_env" => self.handle_execute_query_multi_env(arguments).await,
            "list_environments" => self.handle_list_environments(arguments).await,
            "list_databases_env" => self.handle_list_databases_env(arguments).await,
            "list_databases_all_env" => self.handle_list_databases_all_env(arguments).await,
            "list_tables_env" => self.handle_list_tables_env(arguments).await,
            "describe_table_env" => self.handle_describe_table_env(arguments).await,
            "compare_schema" => self.handle_compare_schema(arguments).await,
            "health_check_env" => self.handle_health_check_env(arguments).await,
            "test_connection_env" => self.handle_test_connection_env(arguments).await,
            "get_monitoring_report" => self.handle_get_monitoring_report(arguments).await,
            "get_performance_metrics" => self.handle_get_performance_metrics(arguments).await,
            "execute_streaming_query_env" => self.handle_execute_streaming_query_env(arguments).await,
            "execute_streaming_query_multi_env" => self.handle_execute_streaming_query_multi_env(arguments).await,
            _ => Err(ServerError::validation_error(
                format!("Unknown enhanced MCP tool: {}", tool_name),
                Some(tool_name.to_string())
            ))
        }
    }

    /// Handle execute_query_env tool
    async fn handle_execute_query_env(&self, arguments: Value) -> Result<Value> {
        let sql = arguments.get("sql")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ServerError::validation_error(
                "Missing required parameter: sql".to_string(),
                None
            ))?
            .to_string();

        let environment = arguments.get("environment")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let parameters = arguments.get("parameters")
            .and_then(|v| v.as_array())
            .cloned();

        let stream_results = arguments.get("stream_results")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Validate that only read-only queries are allowed
        let sql_trimmed = sql.trim().to_uppercase();
        if !Self::is_read_only_query(&sql_trimmed) {
            return Err(ServerError::validation_error(
                "Only read-only queries (SELECT, SHOW, DESCRIBE, EXPLAIN) are allowed".to_string(),
                Some(format!("Rejected query type: {}", sql.chars().take(100).collect::<String>()))
            ));
        }

        let request = EnvQueryRequest {
            sql,
            environment,
            parameters,
            stream_results,
        };

        let result = self.query_router.execute_query(&request).await?;
        
        Ok(serde_json::to_value(result)?)
    }

    /// Handle execute_query_multi_env tool
    async fn handle_execute_query_multi_env(&self, arguments: Value) -> Result<Value> {
        let sql = arguments.get("sql")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ServerError::validation_error(
                "Missing required parameter: sql".to_string(),
                None
            ))?
            .to_string();

        let environments = arguments.get("environments")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_else(Vec::new);

        let parameters = arguments.get("parameters")
            .and_then(|v| v.as_array())
            .cloned();

        let compare_results = arguments.get("compare_results")
            .and_then(|v| v.as_bool());

        // Validate that only read-only queries are allowed
        let sql_trimmed = sql.trim().to_uppercase();
        if !Self::is_read_only_query(&sql_trimmed) {
            return Err(ServerError::validation_error(
                "Only read-only queries (SELECT, SHOW, DESCRIBE, EXPLAIN) are allowed".to_string(),
                Some(format!("Rejected query type: {}", sql.chars().take(100).collect::<String>()))
            ));
        }

        let request = MultiEnvQueryRequest {
            sql,
            environments,
            parameters,
            compare_results,
        };

        let result = self.query_router.execute_multi_env_query(&request).await?;
        
        Ok(serde_json::to_value(result)?)
    }

    /// Handle list_environments tool
    async fn handle_list_environments(&self, arguments: Value) -> Result<Value> {
        let include_disabled = arguments.get("include_disabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let status_report = self.environment_manager.get_environment_status_report();
        
        let environments: Vec<_> = status_report
            .into_iter()
            .filter(|(_, report)| {
                include_disabled || matches!(report.status, crate::environment::EnvironmentStatus::Enabled)
            })
            .map(|(name, report)| {
                json!({
                    "name": name,
                    "description": report.description,
                    "status": match report.status {
                        crate::environment::EnvironmentStatus::Enabled => "enabled",
                        crate::environment::EnvironmentStatus::Disabled => "disabled",
                        crate::environment::EnvironmentStatus::Invalid(_) => "invalid",
                    },
                    "is_default": report.is_default,
                    "is_legacy": report.is_legacy,
                    "connection_info": {
                        "host": report.connection_info.host,
                        "port": report.connection_info.port,
                        "database": report.connection_info.database,
                        "password_configured": report.connection_info.password_configured
                    },
                    "pool_config": {
                        "max_connections": report.pool_config.max_connections,
                        "min_connections": report.pool_config.min_connections,
                        "connection_timeout": report.pool_config.connection_timeout,
                        "idle_timeout": report.pool_config.idle_timeout
                    }
                })
            })
            .collect();

        Ok(json!({
            "environments": environments,
            "total_count": environments.len(),
            "default_environment": self.environment_manager.get_default_environment()
        }))
    }

    /// Handle list_databases_env tool
    async fn handle_list_databases_env(&self, arguments: Value) -> Result<Value> {
        let environment = arguments.get("environment")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let request = EnvQueryRequest {
            sql: "SELECT SCHEMA_NAME AS Database_Name FROM INFORMATION_SCHEMA.SCHEMATA ORDER BY SCHEMA_NAME".to_string(),
            environment,
            parameters: None,
            stream_results: false,
        };

        let result = self.query_router.execute_query(&request).await?;
        
        Ok(serde_json::to_value(result)?)
    }

    /// Handle list_databases_all_env tool
    async fn handle_list_databases_all_env(&self, arguments: Value) -> Result<Value> {
        let environments = arguments.get("environments")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_else(Vec::new);

        let request = MultiEnvQueryRequest {
            sql: "SELECT SCHEMA_NAME AS Database_Name FROM INFORMATION_SCHEMA.SCHEMATA ORDER BY SCHEMA_NAME".to_string(),
            environments,
            parameters: None,
            compare_results: Some(false),
        };

        let result = self.query_router.execute_multi_env_query(&request).await?;
        
        Ok(serde_json::to_value(result)?)
    }

    /// Handle list_tables_env tool
    async fn handle_list_tables_env(&self, arguments: Value) -> Result<Value> {
        let environment = arguments.get("environment")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let database = arguments.get("database")
            .and_then(|v| v.as_str());

        let sql = if let Some(db) = database {
            format!(
                "SELECT TABLE_NAME FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_SCHEMA = '{}' ORDER BY TABLE_NAME",
                db
            )
        } else {
            "SELECT TABLE_NAME FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_SCHEMA = DATABASE() ORDER BY TABLE_NAME".to_string()
        };

        let request = EnvQueryRequest {
            sql,
            environment,
            parameters: None,
            stream_results: false,
        };

        let result = self.query_router.execute_query(&request).await?;
        
        Ok(serde_json::to_value(result)?)
    }

    /// Handle describe_table_env tool
    async fn handle_describe_table_env(&self, arguments: Value) -> Result<Value> {
        let table = arguments.get("table")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ServerError::validation_error(
                "Missing required parameter: table".to_string(),
                None
            ))?;

        let environment = arguments.get("environment")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let database = arguments.get("database")
            .and_then(|v| v.as_str());

        let sql = if let Some(db) = database {
            format!(
                "SELECT COLUMN_NAME as Field, DATA_TYPE as Type, IS_NULLABLE as `Null`, 
                        COLUMN_KEY as `Key`, COLUMN_DEFAULT as `Default`, EXTRA as Extra
                 FROM INFORMATION_SCHEMA.COLUMNS 
                 WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' 
                 ORDER BY ORDINAL_POSITION",
                db, table
            )
        } else {
            format!(
                "SELECT COLUMN_NAME as Field, DATA_TYPE as Type, IS_NULLABLE as `Null`, 
                        COLUMN_KEY as `Key`, COLUMN_DEFAULT as `Default`, EXTRA as Extra
                 FROM INFORMATION_SCHEMA.COLUMNS 
                 WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = '{}' 
                 ORDER BY ORDINAL_POSITION",
                table
            )
        };

        let request = EnvQueryRequest {
            sql,
            environment,
            parameters: None,
            stream_results: false,
        };

        let result = self.query_router.execute_query(&request).await?;
        
        Ok(serde_json::to_value(result)?)
    }

    /// Handle compare_schema tool
    async fn handle_compare_schema(&self, arguments: Value) -> Result<Value> {
        let environments = arguments.get("environments")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ServerError::validation_error(
                "Missing required parameter: environments".to_string(),
                None
            ))?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>();

        if environments.len() < 2 {
            return Err(ServerError::validation_error(
                "At least 2 environments are required for schema comparison".to_string(),
                None
            ));
        }

        let database = arguments.get("database")
            .and_then(|v| v.as_str());

        let table = arguments.get("table")
            .and_then(|v| v.as_str());

        // Validate all environments exist and are enabled
        for env in &environments {
            self.environment_manager.validate_environment(env)?;
        }

        let comparison_result = self.compare_schema_across_environments(&environments, database, table).await?;
        
        Ok(serde_json::to_value(comparison_result)?)
    }

    /// Handle health_check_env tool
    async fn handle_health_check_env(&self, arguments: Value) -> Result<Value> {
        let environment = arguments.get("environment")
            .and_then(|v| v.as_str());

        let comprehensive = arguments.get("comprehensive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let health_status = if let Some(env) = environment {
            // Check specific environment
            self.environment_manager.validate_environment(env)?;
            
            if comprehensive {
                // Perform comprehensive health check
                let detailed_status = self.pool_manager.comprehensive_health_check(env).await?;
                detailed_status
            } else {
                // Perform basic health check
                let status = self.pool_manager.health_check(Some(env)).await?;
                json!({
                    "environment": env,
                    "status": status,
                    "timestamp": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                })
            }
        } else {
            // Check all environments
            if comprehensive {
                // Get comprehensive monitoring report
                self.pool_manager.get_monitoring_report().await
            } else {
                // Basic health check for all environments
                let mut all_status = HashMap::new();
                let mut overall_healthy = true;
                
                for env_name in self.environment_manager.list_enabled_environments() {
                    match self.pool_manager.health_check(Some(env_name)).await {
                        Ok(status) => {
                            // Convert PoolHealthStatus to a serializable format
                            let status_map = status.into_iter().map(|(env, health)| {
                                let is_healthy = matches!(health, crate::pool::PoolHealthStatus::Healthy);
                                if !is_healthy {
                                    overall_healthy = false;
                                }
                                (env, health)
                            }).collect::<HashMap<_, _>>();
                            
                            all_status.insert(env_name.to_string(), serde_json::to_value(status_map)?);
                        }
                        Err(e) => {
                            error!("Health check failed for environment '{}': {}", env_name, e);
                            overall_healthy = false;
                            all_status.insert(env_name.to_string(), json!({
                                "status": "error",
                                "error": e.user_message()
                            }));
                        }
                    }
                }
                
                json!({
                    "environments": all_status,
                    "overall_healthy": overall_healthy,
                    "timestamp": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                })
            }
        };

        Ok(health_status)
    }

    /// Handle test_connection_env tool
    async fn handle_test_connection_env(&self, arguments: Value) -> Result<Value> {
        let environment = arguments.get("environment")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ServerError::validation_error(
                "Missing required parameter: environment".to_string(),
                None
            ))?;

        // Validate environment exists and is enabled
        self.environment_manager.validate_environment(environment)?;

        // Test connection using the pool manager
        let connection_test_result = self.pool_manager.test_connection(environment).await?;
        
        Ok(serde_json::to_value(connection_test_result)?)
    }

    /// Compare schema across multiple environments
    async fn compare_schema_across_environments(
        &self,
        environments: &[String],
        database: Option<&str>,
        table: Option<&str>,
    ) -> Result<SchemaComparisonResult> {
        let mut schemas = HashMap::new();
        let mut differences = Vec::new();

        // Get schema information for each environment
        for env in environments {
            let schema = self.get_schema_info(env, database, table).await?;
            schemas.insert(env.clone(), schema);
        }

        // Compare schemas between environments
        for i in 0..environments.len() {
            for j in (i + 1)..environments.len() {
                let env1 = &environments[i];
                let env2 = &environments[j];
                
                let schema1 = schemas.get(env1).unwrap();
                let schema2 = schemas.get(env2).unwrap();
                
                let mut env_differences = self.compare_table_schemas(env1, env2, schema1, schema2);
                differences.append(&mut env_differences);
            }
        }

        let identical = differences.is_empty();

        Ok(SchemaComparisonResult {
            environments: environments.to_vec(),
            identical,
            differences,
            schemas,
        })
    }

    /// Get schema information for an environment
    async fn get_schema_info(
        &self,
        environment: &str,
        database: Option<&str>,
        table: Option<&str>,
    ) -> Result<Vec<TableSchema>> {
        let mut schemas = Vec::new();

        // Get list of tables
        let tables_sql = if let Some(db) = database {
            if let Some(tbl) = table {
                format!(
                    "SELECT TABLE_NAME FROM INFORMATION_SCHEMA.TABLES 
                     WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' 
                     ORDER BY TABLE_NAME",
                    db, tbl
                )
            } else {
                format!(
                    "SELECT TABLE_NAME FROM INFORMATION_SCHEMA.TABLES 
                     WHERE TABLE_SCHEMA = '{}' 
                     ORDER BY TABLE_NAME",
                    db
                )
            }
        } else {
            if let Some(tbl) = table {
                format!(
                    "SELECT TABLE_NAME FROM INFORMATION_SCHEMA.TABLES 
                     WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = '{}' 
                     ORDER BY TABLE_NAME",
                    tbl
                )
            } else {
                "SELECT TABLE_NAME FROM INFORMATION_SCHEMA.TABLES 
                 WHERE TABLE_SCHEMA = DATABASE() 
                 ORDER BY TABLE_NAME".to_string()
            }
        };

        let tables_request = EnvQueryRequest {
            sql: tables_sql,
            environment: Some(environment.to_string()),
            parameters: None,
            stream_results: false,
        };

        let tables_result = self.query_router.execute_query(&tables_request).await?;
        
        if let Some(error) = &tables_result.error {
            return Err(ServerError::internal_error(
                format!("Failed to get table list for environment '{}': {}", environment, error),
                None
            ));
        }

        // For each table, get detailed schema information
        for row in &tables_result.rows {
            if let Some(table_name_value) = row.values.get(0) {
                if let Some(table_name) = table_name_value.as_str() {
                    let table_schema = self.get_table_schema_info(
                        environment,
                        database,
                        table_name,
                    ).await?;
                    schemas.push(table_schema);
                }
            }
        }

        Ok(schemas)
    }

    /// Get detailed schema information for a specific table
    async fn get_table_schema_info(
        &self,
        environment: &str,
        database: Option<&str>,
        table_name: &str,
    ) -> Result<TableSchema> {
        // Get column information
        let columns = self.get_column_schema_info(environment, database, table_name).await?;
        
        // Get index information
        let indexes = self.get_index_schema_info(environment, database, table_name).await?;
        
        // Get constraint information
        let constraints = self.get_constraint_schema_info(environment, database, table_name).await?;

        let database_name = database.unwrap_or("current_database").to_string();

        Ok(TableSchema {
            table_name: table_name.to_string(),
            database_name,
            columns,
            indexes,
            constraints,
        })
    }

    /// Get column schema information for a table
    async fn get_column_schema_info(
        &self,
        environment: &str,
        database: Option<&str>,
        table_name: &str,
    ) -> Result<Vec<ColumnSchema>> {
        let sql = if let Some(db) = database {
            format!(
                "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT, 
                        COLUMN_KEY, EXTRA, COLUMN_COMMENT
                 FROM INFORMATION_SCHEMA.COLUMNS 
                 WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' 
                 ORDER BY ORDINAL_POSITION",
                db, table_name
            )
        } else {
            format!(
                "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT, 
                        COLUMN_KEY, EXTRA, COLUMN_COMMENT
                 FROM INFORMATION_SCHEMA.COLUMNS 
                 WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = '{}' 
                 ORDER BY ORDINAL_POSITION",
                table_name
            )
        };

        let request = EnvQueryRequest {
            sql,
            environment: Some(environment.to_string()),
            parameters: None,
            stream_results: false,
        };

        let result = self.query_router.execute_query(&request).await?;
        
        if let Some(error) = &result.error {
            return Err(ServerError::internal_error(
                format!("Failed to get column info for table '{}' in environment '{}': {}", 
                        table_name, environment, error),
                None
            ));
        }

        let mut columns = Vec::new();
        
        for row in &result.rows {
            if row.values.len() >= 6 {
                let name = row.values[0].as_str().unwrap_or("").to_string();
                let data_type = row.values[1].as_str().unwrap_or("").to_string();
                let nullable = row.values[2].as_str().unwrap_or("NO") == "YES";
                let default_value = row.values[3].as_str().map(|s| s.to_string());
                let column_key = row.values[4].as_str().unwrap_or("");
                let extra = row.values[5].as_str().unwrap_or("");
                let comment = if row.values.len() > 6 {
                    row.values[6].as_str().map(|s| s.to_string())
                } else {
                    None
                };

                let is_primary_key = column_key.contains("PRI");
                let is_auto_increment = extra.contains("auto_increment");

                columns.push(ColumnSchema {
                    name,
                    data_type,
                    nullable,
                    default_value,
                    is_primary_key,
                    is_auto_increment,
                    comment,
                });
            }
        }

        Ok(columns)
    }

    /// Get index schema information for a table
    async fn get_index_schema_info(
        &self,
        environment: &str,
        database: Option<&str>,
        table_name: &str,
    ) -> Result<Vec<IndexSchema>> {
        let sql = if let Some(db) = database {
            format!(
                "SELECT INDEX_NAME, NON_UNIQUE, COLUMN_NAME, INDEX_TYPE
                 FROM INFORMATION_SCHEMA.STATISTICS 
                 WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' 
                 ORDER BY INDEX_NAME, SEQ_IN_INDEX",
                db, table_name
            )
        } else {
            format!(
                "SELECT INDEX_NAME, NON_UNIQUE, COLUMN_NAME, INDEX_TYPE
                 FROM INFORMATION_SCHEMA.STATISTICS 
                 WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = '{}' 
                 ORDER BY INDEX_NAME, SEQ_IN_INDEX",
                table_name
            )
        };

        let request = EnvQueryRequest {
            sql,
            environment: Some(environment.to_string()),
            parameters: None,
            stream_results: false,
        };

        let result = self.query_router.execute_query(&request).await?;
        
        if let Some(error) = &result.error {
            return Err(ServerError::internal_error(
                format!("Failed to get index info for table '{}' in environment '{}': {}", 
                        table_name, environment, error),
                None
            ));
        }

        let mut index_map: HashMap<String, (bool, Vec<String>, String)> = HashMap::new();
        
        for row in &result.rows {
            if row.values.len() >= 4 {
                let index_name = row.values[0].as_str().unwrap_or("").to_string();
                let non_unique = row.values[1].as_i64().unwrap_or(1) != 0;
                let column_name = row.values[2].as_str().unwrap_or("").to_string();
                let index_type = row.values[3].as_str().unwrap_or("").to_string();

                let entry = index_map.entry(index_name.clone()).or_insert((
                    !non_unique, // is_unique is opposite of non_unique
                    Vec::new(),
                    index_type.clone(),
                ));
                entry.1.push(column_name);
            }
        }

        let indexes = index_map
            .into_iter()
            .map(|(name, (is_unique, columns, index_type))| IndexSchema {
                name,
                is_unique,
                columns,
                index_type,
            })
            .collect();

        Ok(indexes)
    }

    /// Get constraint schema information for a table
    async fn get_constraint_schema_info(
        &self,
        environment: &str,
        database: Option<&str>,
        table_name: &str,
    ) -> Result<Vec<ConstraintSchema>> {
        let sql = if let Some(db) = database {
            format!(
                "SELECT CONSTRAINT_NAME, CONSTRAINT_TYPE, COLUMN_NAME,
                        REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME
                 FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE kcu
                 JOIN INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc 
                   ON kcu.CONSTRAINT_NAME = tc.CONSTRAINT_NAME 
                   AND kcu.TABLE_SCHEMA = tc.TABLE_SCHEMA
                 WHERE kcu.TABLE_SCHEMA = '{}' AND kcu.TABLE_NAME = '{}' 
                 ORDER BY CONSTRAINT_NAME, ORDINAL_POSITION",
                db, table_name
            )
        } else {
            format!(
                "SELECT CONSTRAINT_NAME, CONSTRAINT_TYPE, COLUMN_NAME,
                        REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME
                 FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE kcu
                 JOIN INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc 
                   ON kcu.CONSTRAINT_NAME = tc.CONSTRAINT_NAME 
                   AND kcu.TABLE_SCHEMA = tc.TABLE_SCHEMA
                 WHERE kcu.TABLE_SCHEMA = DATABASE() AND kcu.TABLE_NAME = '{}' 
                 ORDER BY CONSTRAINT_NAME, ORDINAL_POSITION",
                table_name
            )
        };

        let request = EnvQueryRequest {
            sql,
            environment: Some(environment.to_string()),
            parameters: None,
            stream_results: false,
        };

        let result = self.query_router.execute_query(&request).await?;
        
        if let Some(error) = &result.error {
            // Constraints query might fail on some MySQL versions, return empty list
            info!("Could not get constraint info for table '{}' in environment '{}': {}", 
                  table_name, environment, error);
            return Ok(Vec::new());
        }

        let mut constraint_map: HashMap<String, (String, Vec<String>, Option<String>, Vec<String>)> = HashMap::new();
        
        for row in &result.rows {
            if row.values.len() >= 3 {
                let constraint_name = row.values[0].as_str().unwrap_or("").to_string();
                let constraint_type = row.values[1].as_str().unwrap_or("").to_string();
                let column_name = row.values[2].as_str().unwrap_or("").to_string();
                let referenced_table = row.values.get(3).and_then(|v| v.as_str()).map(|s| s.to_string());
                let referenced_column = row.values.get(4).and_then(|v| v.as_str()).unwrap_or("").to_string();

                let entry = constraint_map.entry(constraint_name.clone()).or_insert((
                    constraint_type.clone(),
                    Vec::new(),
                    referenced_table.clone(),
                    Vec::new(),
                ));
                entry.1.push(column_name);
                if !referenced_column.is_empty() {
                    entry.3.push(referenced_column);
                }
            }
        }

        let constraints = constraint_map
            .into_iter()
            .map(|(name, (constraint_type, columns, referenced_table, referenced_columns))| {
                ConstraintSchema {
                    name,
                    constraint_type,
                    columns,
                    referenced_table,
                    referenced_columns: if referenced_columns.is_empty() {
                        None
                    } else {
                        Some(referenced_columns)
                    },
                }
            })
            .collect();

        Ok(constraints)
    }

    /// Compare table schemas between two environments
    fn compare_table_schemas(
        &self,
        env1: &str,
        env2: &str,
        schema1: &[TableSchema],
        schema2: &[TableSchema],
    ) -> Vec<SchemaDifference> {
        let mut differences = Vec::new();

        // Create maps for easier lookup
        let schema1_map: HashMap<&str, &TableSchema> = schema1
            .iter()
            .map(|table| (table.table_name.as_str(), table))
            .collect();

        let schema2_map: HashMap<&str, &TableSchema> = schema2
            .iter()
            .map(|table| (table.table_name.as_str(), table))
            .collect();

        // Check for tables that exist in one environment but not the other
        for table_name in schema1_map.keys() {
            if !schema2_map.contains_key(table_name) {
                differences.push(SchemaDifference {
                    difference_type: SchemaDifferenceType::TableExistence,
                    object_name: table_name.to_string(),
                    environments: vec![env1.to_string(), env2.to_string()],
                    description: format!("Table '{}' exists in {} but not in {}", table_name, env1, env2),
                    details: None,
                });
            }
        }

        for table_name in schema2_map.keys() {
            if !schema1_map.contains_key(table_name) {
                differences.push(SchemaDifference {
                    difference_type: SchemaDifferenceType::TableExistence,
                    object_name: table_name.to_string(),
                    environments: vec![env1.to_string(), env2.to_string()],
                    description: format!("Table '{}' exists in {} but not in {}", table_name, env2, env1),
                    details: None,
                });
            }
        }

        // Compare tables that exist in both environments
        for (table_name, table1) in &schema1_map {
            if let Some(table2) = schema2_map.get(table_name) {
                let mut table_differences = self.compare_single_table_schema(env1, env2, table1, table2);
                differences.append(&mut table_differences);
            }
        }

        differences
    }

    /// Compare schema of a single table between two environments
    fn compare_single_table_schema(
        &self,
        env1: &str,
        env2: &str,
        table1: &TableSchema,
        table2: &TableSchema,
    ) -> Vec<SchemaDifference> {
        let mut differences = Vec::new();

        // Compare columns
        let columns1_map: HashMap<&str, &ColumnSchema> = table1
            .columns
            .iter()
            .map(|col| (col.name.as_str(), col))
            .collect();

        let columns2_map: HashMap<&str, &ColumnSchema> = table2
            .columns
            .iter()
            .map(|col| (col.name.as_str(), col))
            .collect();

        // Check for column differences
        for (col_name, col1) in &columns1_map {
            if let Some(col2) = columns2_map.get(col_name) {
                if col1.data_type != col2.data_type 
                    || col1.nullable != col2.nullable 
                    || col1.is_primary_key != col2.is_primary_key 
                    || col1.is_auto_increment != col2.is_auto_increment {
                    differences.push(SchemaDifference {
                        difference_type: SchemaDifferenceType::ColumnDifference,
                        object_name: format!("{}.{}", table1.table_name, col_name),
                        environments: vec![env1.to_string(), env2.to_string()],
                        description: format!("Column '{}' has different properties between environments", col_name),
                        details: Some(json!({
                            env1: {
                                "data_type": col1.data_type,
                                "nullable": col1.nullable,
                                "is_primary_key": col1.is_primary_key,
                                "is_auto_increment": col1.is_auto_increment
                            },
                            env2: {
                                "data_type": col2.data_type,
                                "nullable": col2.nullable,
                                "is_primary_key": col2.is_primary_key,
                                "is_auto_increment": col2.is_auto_increment
                            }
                        })),
                    });
                }
            } else {
                differences.push(SchemaDifference {
                    difference_type: SchemaDifferenceType::ColumnDifference,
                    object_name: format!("{}.{}", table1.table_name, col_name),
                    environments: vec![env1.to_string(), env2.to_string()],
                    description: format!("Column '{}' exists in {} but not in {}", col_name, env1, env2),
                    details: None,
                });
            }
        }

        for col_name in columns2_map.keys() {
            if !columns1_map.contains_key(col_name) {
                differences.push(SchemaDifference {
                    difference_type: SchemaDifferenceType::ColumnDifference,
                    object_name: format!("{}.{}", table1.table_name, col_name),
                    environments: vec![env1.to_string(), env2.to_string()],
                    description: format!("Column '{}' exists in {} but not in {}", col_name, env2, env1),
                    details: None,
                });
            }
        }

        // Compare indexes (simplified comparison)
        if table1.indexes.len() != table2.indexes.len() {
            differences.push(SchemaDifference {
                difference_type: SchemaDifferenceType::IndexDifference,
                object_name: table1.table_name.clone(),
                environments: vec![env1.to_string(), env2.to_string()],
                description: format!("Table '{}' has different number of indexes", table1.table_name),
                details: Some(json!({
                    env1: table1.indexes.len(),
                    env2: table2.indexes.len()
                })),
            });
        }

        // Compare constraints (simplified comparison)
        if table1.constraints.len() != table2.constraints.len() {
            differences.push(SchemaDifference {
                difference_type: SchemaDifferenceType::ConstraintDifference,
                object_name: table1.table_name.clone(),
                environments: vec![env1.to_string(), env2.to_string()],
                description: format!("Table '{}' has different number of constraints", table1.table_name),
                details: Some(json!({
                    env1: table1.constraints.len(),
                    env2: table2.constraints.len()
                })),
            });
        }

        differences
    }

    /// Handle get_monitoring_report tool
    async fn handle_get_monitoring_report(&self, _arguments: Value) -> Result<Value> {
        info!("Getting comprehensive monitoring report for all environments");
        
        let monitoring_report = self.pool_manager.get_monitoring_report().await;
        
        Ok(monitoring_report)
    }

    /// Handle get_performance_metrics tool
    async fn handle_get_performance_metrics(&self, arguments: Value) -> Result<Value> {
        let environment = arguments.get("environment")
            .and_then(|v| v.as_str());

        if let Some(env) = environment {
            // Get metrics for specific environment
            self.environment_manager.validate_environment(env)?;
            
            let stats = self.pool_manager.get_pool_stats(env).await?;
            let health_status = self.pool_manager.health_check(Some(env)).await?;
            
            Ok(json!({
                "environment": env,
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                "performance_metrics": stats,
                "health_status": health_status,
                "metrics_summary": {
                    "connection_success_rate": stats.connection_success_rate,
                    "query_success_rate": stats.query_success_rate,
                    "avg_query_time_ms": stats.avg_query_time_ms,
                    "total_queries": stats.total_successful_queries + stats.total_failed_queries,
                    "total_connections": stats.total_connections_created + stats.total_connection_failures
                }
            }))
        } else {
            // Get metrics for all environments
            let mut all_metrics = HashMap::new();
            let mut summary = json!({
                "total_environments": 0,
                "avg_connection_success_rate": 0.0,
                "avg_query_success_rate": 0.0,
                "avg_query_time_ms": 0.0,
                "total_queries_all_envs": 0,
                "total_connections_all_envs": 0
            });
            
            let mut total_conn_rate = 0.0;
            let mut total_query_rate = 0.0;
            let mut total_query_time = 0.0;
            let mut env_count = 0;
            let mut total_queries = 0u64;
            let mut total_connections = 0u64;
            
            for env_name in self.environment_manager.list_enabled_environments() {
                match self.pool_manager.get_pool_stats(env_name).await {
                    Ok(stats) => {
                        let health_status = self.pool_manager.health_check(Some(env_name)).await
                            .unwrap_or_else(|_| HashMap::new());
                        
                        all_metrics.insert(env_name.to_string(), json!({
                            "performance_metrics": stats,
                            "health_status": health_status,
                            "metrics_summary": {
                                "connection_success_rate": stats.connection_success_rate,
                                "query_success_rate": stats.query_success_rate,
                                "avg_query_time_ms": stats.avg_query_time_ms,
                                "total_queries": stats.total_successful_queries + stats.total_failed_queries,
                                "total_connections": stats.total_connections_created + stats.total_connection_failures
                            }
                        }));
                        
                        // Accumulate for summary
                        total_conn_rate += stats.connection_success_rate;
                        total_query_rate += stats.query_success_rate;
                        total_query_time += stats.avg_query_time_ms;
                        total_queries += stats.total_successful_queries + stats.total_failed_queries;
                        total_connections += stats.total_connections_created + stats.total_connection_failures;
                        env_count += 1;
                    }
                    Err(e) => {
                        error!("Failed to get metrics for environment '{}': {}", env_name, e);
                        all_metrics.insert(env_name.to_string(), json!({
                            "error": e.user_message()
                        }));
                    }
                }
            }
            
            // Calculate averages
            if env_count > 0 {
                summary["total_environments"] = json!(env_count);
                summary["avg_connection_success_rate"] = json!(total_conn_rate / env_count as f64);
                summary["avg_query_success_rate"] = json!(total_query_rate / env_count as f64);
                summary["avg_query_time_ms"] = json!(total_query_time / env_count as f64);
                summary["total_queries_all_envs"] = json!(total_queries);
                summary["total_connections_all_envs"] = json!(total_connections);
            }
            
            Ok(json!({
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                "summary": summary,
                "environments": all_metrics
            }))
        }
    }

    /// Handle execute_streaming_query_env tool
    async fn handle_execute_streaming_query_env(&self, arguments: Value) -> Result<Value> {
        let sql = arguments.get("sql")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ServerError::validation_error(
                "Missing required parameter: sql".to_string(),
                None
            ))?
            .to_string();

        let environment = arguments.get("environment")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let parameters = arguments.get("parameters")
            .and_then(|v| v.as_array())
            .cloned();

        // Validate that only read-only queries are allowed
        let sql_trimmed = sql.trim().to_uppercase();
        if !Self::is_read_only_query(&sql_trimmed) {
            return Err(ServerError::validation_error(
                "Only read-only queries (SELECT, SHOW, DESCRIBE, EXPLAIN) are allowed".to_string(),
                Some(format!("Rejected query type: {}", sql.chars().take(100).collect::<String>()))
            ));
        }

        let request = crate::router::EnvQueryRequest {
            sql,
            environment,
            parameters,
            stream_results: true, // Force streaming
        };

        let result = self.query_router.execute_streaming_query_env(&request).await?;
        
        Ok(serde_json::to_value(result)?)
    }

    /// Handle execute_streaming_query_multi_env tool
    async fn handle_execute_streaming_query_multi_env(&self, arguments: Value) -> Result<Value> {
        let sql = arguments.get("sql")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ServerError::validation_error(
                "Missing required parameter: sql".to_string(),
                None
            ))?
            .to_string();

        let environments = arguments.get("environments")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_else(Vec::new);

        let parameters = arguments.get("parameters")
            .and_then(|v| v.as_array())
            .cloned();

        // Validate that only read-only queries are allowed
        let sql_trimmed = sql.trim().to_uppercase();
        if !Self::is_read_only_query(&sql_trimmed) {
            return Err(ServerError::validation_error(
                "Only read-only queries (SELECT, SHOW, DESCRIBE, EXPLAIN) are allowed".to_string(),
                Some(format!("Rejected query type: {}", sql.chars().take(100).collect::<String>()))
            ));
        }

        let request = crate::router::MultiEnvQueryRequest {
            sql,
            environments,
            parameters,
            compare_results: Some(false), // Don't compare for streaming
        };

        let result = self.query_router.execute_multi_env_streaming_query(&request).await?;
        
        Ok(serde_json::to_value(result)?)
    }

    /// Check if a SQL query is read-only
    fn is_read_only_query(sql: &str) -> bool {
        let sql_trimmed = sql.trim().to_uppercase();
        
        sql_trimmed.starts_with("SELECT") 
            || sql_trimmed.starts_with("SHOW") 
            || sql_trimmed.starts_with("DESCRIBE") 
            || sql_trimmed.starts_with("DESC") 
            || sql_trimmed.starts_with("EXPLAIN")
    }
}