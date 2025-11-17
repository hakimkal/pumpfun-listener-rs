use anyhow::Result;

 use tracing::{info, error};
use crate::config::Config;
use crate::models::{Event, Token};

#[derive(Clone)]
pub struct Processor {
config: Config
}

impl Processor {
    pub fn new(config: Config) -> Self {
        Self {config  }
    }


    pub async fn process_token_discovered(&self, token: Token) -> Result<()> {




        info!(
            "New token discovered: {} ({}) from {:?}",
            token.symbol.as_deref().unwrap_or("UNKNOWN"),
            token.mint_address,
            token.source
        );


        // Publish event
        self.publish_event(Event::TokenDiscovered(token)).await?;

        Ok(())
    }
    pub async fn process_token_graduated(
        &self,
        token_address: String,
        pair_address: String,
    ) -> Result<()> {
        // Update database with DEX pair information
        info!("Token {} graduated to pair {}", token_address, pair_address);
        // Your database update logic here
        Ok(())
    }


    async fn publish_event(&self, event: Event) -> Result<()> {
        // Publish to Redis pub/sub for other services to consume
        let client = redis::Client::open(self.config.database.redis_url.clone())?;
        let mut conn = client.get_async_connection().await?;

        let event_json = serde_json::to_string(&event)?;
        let _: () = redis::cmd("PUBLISH")
            .arg("events")
            .arg(event_json)
            .query_async(&mut conn)
            .await?;

        Ok(())
    }
}