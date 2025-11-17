
mod listener_helpers;
mod listeners;
mod config;
mod processor;
mod  token_helper;
mod housekeeping_util;
pub mod models;

use std::sync::Arc;
use anyhow::Result;
use tokio::sync::Semaphore;
use tracing::log::info;
use crate::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    housekeeping_util::init_logging();

    housekeeping_util::spawn_log_cleaner( 1);
    info!("Starting Ingestion Service");
    let limiter = Arc::new(Semaphore::new(8));

    // Load config
    let config = Config::load()?;

       // Create processor
    let processor = processor::Processor::new( config.clone());

    // Start listeners

    let pumpfun_listener = listeners::pumpfun::PumpFunListener::new(config.clone(), processor.clone(),limiter.clone());


    // Run  in parallel
    tokio::select! {
        result = pumpfun_listener.start() => {
            tracing::error!("PumpFun listener stopped: {:?}", result);
        }

    }

    Ok(())
}