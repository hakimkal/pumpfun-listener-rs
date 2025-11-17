use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkConfig {
    pub rpc_http_url: String,
    pub rpc_wss_url: String,

    pub commitment: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct IngestionConfig {
    pub min_tx_count_for_active_pair: u32,
}





#[derive(Debug, Clone, Deserialize)]
pub struct ProgramsConfig {

    pub pump_fun: String,
    pub token_program: String,

}



#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
     pub redis_url: String,
}



#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub network: NetworkConfig,
    pub database: DatabaseConfig,

    pub programs: ProgramsConfig,
    pub ingestion: IngestionConfig,

    pub api: ApiConfig,
}
impl Config {


    pub fn load() -> Result<Self> {
        // Load .env file first (sets environment variables)
        dotenv::dotenv().ok();

        // Build config from multiple sources (later sources override earlier ones)
        let config = config::Config::builder()
            // 1. Start with config.toml (base configuration)
            .add_source(config::File::with_name("config/config").required(false))
            // 2. Override with environment-specific config (if exists)
            .add_source(
                config::File::with_name(&format!(
                    "config/config.{}",
                    std::env::var("ENV").unwrap_or_else(|_| "dev".to_string())
                ))
                .required(false),
            )
            // 3. Override with environment variables (no prefix, use __ as separator)
            .add_source(
                config::Environment::default()
                    .separator("__")
                    .try_parsing(true),
            )
            .build()
            .context("Failed to build configuration")?;

        config
            .try_deserialize()
            .context("Failed to deserialize configuration. Check your config.toml and .env files")
    }
}