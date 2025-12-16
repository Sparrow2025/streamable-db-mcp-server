//! Environment Manager demonstration
//! 
//! This example shows how to use the EnvironmentManager to load and manage
//! multiple database environments.

use mysql_mcp_server::{Config, EnvironmentManager};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Environment Manager Demo");
    println!("==========================");

    // Load configuration from the example multi-environment config
    let config = Config::from_file("config.multi-env.example.toml")?;
    println!("✅ Configuration loaded successfully");

    // Create Environment Manager
    let env_manager = EnvironmentManager::load_from_config(&config)?;
    println!("✅ Environment Manager initialized");

    // Display basic information
    println!("\n📊 Environment Summary:");
    println!("  - Total environments: {}", env_manager.environment_count());
    println!("  - Enabled environments: {}", env_manager.enabled_environment_count());
    println!("  - Legacy mode: {}", env_manager.is_legacy_mode());
    
    if let Some(default_env) = env_manager.get_default_environment() {
        println!("  - Default environment: {}", default_env);
    }

    // List all environments
    println!("\n🌍 All Environments:");
    for env_name in env_manager.list_environments() {
        println!("  - {}", env_name);
    }

    // List enabled environments
    println!("\n✅ Enabled Environments:");
    for env_name in env_manager.list_enabled_environments() {
        println!("  - {}", env_name);
    }

    // Get detailed status report
    println!("\n📋 Detailed Environment Status:");
    let status_report = env_manager.get_environment_status_report();
    
    for (env_name, report) in &status_report {
        println!("\n  Environment: {}", env_name);
        println!("    Description: {:?}", report.description);
        println!("    Status: {:?}", report.status);
        println!("    Is Default: {}", report.is_default);
        println!("    Connection: {}@{}:{}/{}", 
                 report.connection_info.username,
                 report.connection_info.host,
                 report.connection_info.port,
                 report.connection_info.database);
        println!("    Password Configured: {}", report.connection_info.password_configured);
        println!("    Pool Config: {}-{} connections, {}s timeout", 
                 report.pool_config.min_connections,
                 report.pool_config.max_connections,
                 report.pool_config.connection_timeout);
    }

    // Test environment validation
    println!("\n🔍 Environment Validation:");
    for env_name in env_manager.list_environments() {
        match env_manager.validate_environment(env_name) {
            Ok(()) => println!("  ✅ {} - Valid", env_name),
            Err(err) => println!("  ❌ {} - Error: {}", env_name, err),
        }
    }

    // Test connection URL generation (masked for security)
    println!("\n🔗 Connection URLs (masked):");
    for env_name in env_manager.list_environments() {
        if let Some(masked_url) = env_manager.get_masked_connection_url(env_name) {
            println!("  {}: {}", env_name, masked_url);
        }
    }

    println!("\n🎉 Environment Manager demo completed successfully!");
    Ok(())
}