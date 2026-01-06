use builder_derive::Builder;
use std::collections::HashMap;

#[derive(Builder, Debug)]
struct ServerConfig {
    host: String,
    port: u16,
    workers: usize,
    features: Vec<String>,
    middlewares: Vec<String>,
    timeout: Option<u64>,
}

#[derive(Builder, Debug)]
struct DatabaseConfig {
    connection_string: String,
    max_connections: u32,
    tables: Vec<String>,
    options: Option<String>,
}

fn main() {
    println!("=== Complex Types Example ===\n");

    // Server config with Vec fields
    let server = ServerConfig::builder()
        .host("0.0.0.0".to_string())
        .port(8080)
        .workers(4)
        .features(vec![
            "logging".to_string(),
            "compression".to_string(),
            "caching".to_string(),
        ])
        .middlewares(vec!["auth".to_string(), "cors".to_string()])
        .timeout(30000)
        .build()
        .expect("Failed to build server config");

    println!("Server config: {:?}", server);

    // Database config with empty Vec (using default)
    let db = DatabaseConfig::builder()
        .connection_string("postgresql://localhost/mydb".to_string())
        .max_connections(10)
        .build()
        .expect("Failed to build database config");

    println!("\nDatabase config (with empty tables): {:?}", db);

    // Database config with tables specified
    let db_with_tables = DatabaseConfig::builder()
        .connection_string("postgresql://localhost/mydb".to_string())
        .max_connections(10)
        .tables(vec![
            "users".to_string(),
            "posts".to_string(),
            "comments".to_string(),
        ])
        .options("ssl=true".to_string())
        .build()
        .expect("Failed to build database config");

    println!("\nDatabase config (with tables): {:?}", db_with_tables);
}
