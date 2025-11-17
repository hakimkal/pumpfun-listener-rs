use crate::processor::Processor;
use crate::{listener_helpers, token_helper};
use anyhow::{Context, Result};
use bigdecimal::{BigDecimal, FromPrimitive, Zero};
use chrono::TimeZone;
use futures::StreamExt;
use std::clone;

use crate::config::Config;
use crate::models::{Token, TokenSource};
use solana_client::rpc_client::RpcClient;
use solana_client::{
    nonblocking::pubsub_client::PubsubClient,
    rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
    rpc_response::RpcLogsResponse,
};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction, UiInstruction, UiMessage,
    UiParsedInstruction,
};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

pub struct PumpFunListener {
    config: Config,
    processor: Processor,
    limiter: Arc<Semaphore>,
}

impl PumpFunListener {
    pub fn new(config: Config, processor: Processor, limiter: Arc<Semaphore>) -> Self {
        Self {
            config,
            processor,
            limiter,
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting Pump.fun listener");

        loop {
            if let Err(e) = self.listen().await {
                error!("Pump.fun listener error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }

    async fn listen(&self) -> Result<()> {
        let pubsub = PubsubClient::new(&self.config.network.rpc_wss_url).await?;

        let pumpfun_pubkey = Pubkey::from_str(&self.config.programs.pump_fun)?;

        let (mut stream, unsubscribe) = pubsub
            .logs_subscribe(
                RpcTransactionLogsFilter::Mentions(vec![pumpfun_pubkey.to_string()]),
                // RpcTransactionLogsFilter::All,
                RpcTransactionLogsConfig {
                    commitment: Some(CommitmentConfig::confirmed()),
                },
            )
            .await?;

        info!("Subscribed to Pump.fun program");

        while let Some(result) = stream.next().await {
            let rpc_log: RpcLogsResponse = result.value;

            if rpc_log
                .logs
                .iter()
                .any(|l| l.contains(&pumpfun_pubkey.to_string()))
            {
                if let Err(e) = self.process_log(rpc_log.clone()).await {
                    // error!("Pump.fun process_log error: {:?}", rpc_log);
                    error!("Error processing Pump.fun log: {}", e);
                }
            }
        }

        unsubscribe().await;
        Ok(())
    }

    pub async fn process_log(&self, log: RpcLogsResponse) -> Result<()> {
        // Check if transaction succeeded
        let is_success = log.logs.iter().any(|l| l.contains("success"));
        if !is_success {
            return Ok(());
        }

        // Detect buy/sell instructions
        let is_buy = log.logs.iter().any(|l| l.contains("Instruction: Buy"));
        let is_sell = log.logs.iter().any(|l| l.contains("Instruction: Sell"));

        if is_buy {
            info!("Detected Pump.fun Buy: {:?}", &log.signature);
        }
        if is_sell {
            info!("Detected Pump.fun Sell: {:?}", &log.signature);
        }

        // Detect token creation
        let is_create = log.logs.iter().any(|line| {
            line.contains("InitializeMint")
                || line.contains("InitializeMint2")
                || line.contains("CreateMetadataAccount")
                || line.contains("CreateMetadataAccountV3")
                || line.contains("Instruction: Create")
                || line.contains("master_edition")
                || line.contains("InitializeAccount3")
        });

        // Detect swap events
        let is_swap = log
            .logs
            .iter()
            .any(|l| l.contains("Instruction: SwapTob") || l.contains("SwapEvent"));
        if is_swap {
            info!("Detected Pump.fun Swap: {:?}", &log.signature);
        }
        if !is_create {
            return Ok(());
        }

        info!("Detected new Pump.fun token: {}", log.signature);
        // info!("Full logs for debugging: {:?}", &log.logs);

        let token = self.parse_pumpfun_creation(&log).await?;
        info!("Pump.fun parsed token: {:?}", token);

        if let Some(token) = token {
            self.processor.process_token_discovered(token).await?;
        }

        Ok(())
    }

    pub async fn parse_pumpfun_creation(&self, log: &RpcLogsResponse) -> Result<Option<Token>> {
        let sig = log
            .signature
            .parse()
            .context("Failed to parse transaction signature for pumfun listener")?;
        let rpc = RpcClient::new_with_commitment(
            &self.config.network.rpc_http_url,
            CommitmentConfig::confirmed(),
        );

        // 1️⃣ Fetch the transaction with retry logic
        let tx_opt: Option<EncodedConfirmedTransactionWithStatusMeta> =
            listener_helpers::fetch_transaction_with_retry(&rpc, &sig, self.limiter.clone())
                .await?;

        let tx = match tx_opt {
            Some(tx) => tx,
            None => return Ok(None),
        };

        // 2️⃣ Extract mint address from instructions
        let mut mint_address: Option<Pubkey> = None;

        if let EncodedTransaction::Json(ui_tx) = &tx.transaction.transaction {
            if let UiMessage::Parsed(parsed_msg) = &ui_tx.message {
                for instr in &parsed_msg.instructions {
                    if let UiInstruction::Parsed(UiParsedInstruction::Parsed(pi)) = instr {
                        if pi.program == "spl-associated-token-account"
                            && pi.program_id == "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
                            && pi
                                .parsed
                                .get("type")
                                .unwrap_or(&serde_json::Value::String("".into()))
                                == "createIdempotent"
                        {
                            if let Some(info) = pi.parsed.get("info") {
                                if let Some(mint_str) = info.get("mint").and_then(|m| m.as_str()) {
                                    if let Ok(pubkey) = Pubkey::from_str(mint_str) {
                                        mint_address = Some(pubkey);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mint = match mint_address {
            Some(m) => m,
            None => return Ok(None),
        };

        if mint.to_string() == "So11111111111111111111111111111111111111112" {
            return Ok(None); // skip system accounts
        }
        // 3️⃣ Fetch mint creation transaction (with retry)
        let first_sig_opt = {
            let mut sig_opt: Option<_> = None;
            for attempt in 1..=3 {
                match rpc.get_signatures_for_address(&mint) {
                    Ok(mut sigs) if !sigs.is_empty() => {
                        sig_opt = sigs.last().cloned();
                        break;
                    }
                    Ok(_) => warn!(
                        "No signatures found for mint {} (attempt {}/3)",
                        mint, attempt
                    ),
                    Err(err) => warn!("Failed get_signatures_for_address {}: {}", mint, err),
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            sig_opt
        };

        let created_at = if let Some(sig_info) = first_sig_opt {
            let tx_opt = listener_helpers::fetch_transaction_with_retry(
                &rpc,
                &sig_info.signature.parse()?,
                self.limiter.clone(),
            )
            .await?;

            match tx_opt {
                Some(tx) => tx.block_time.map(|ts| chrono::Utc.timestamp(ts, 0)),
                None => None,
            }
        } else {
            None
        };
        let created_at = created_at.unwrap_or(chrono::Utc::now());

        // 4️⃣ Load mint info and token metadata
        let mint_data = token_helper::load_mint_info(&rpc, &mint)?;
        let token_info =
            token_helper::fetch_token_info("pumpfun", &mint.to_string(), "solana").await?;

        let mint_data = match mint_data {
            Some(m) => m,
            None => return Ok(None),
        };

        Ok(Some(Token {
            mint_address: mint.to_string(),
            created_at,
            discovered_at: chrono::Utc::now(),
            source: TokenSource::Pumpfun,
            name: Some(token_info.name),
            symbol: Some(token_info.symbol),
            decimals: mint_data.decimals,
            total_supply: BigDecimal::from(mint_data.supply),
            holder_count: Some(0),
            top_10_holder_percentage: Some(BigDecimal::zero()),
            liquidity_sol: Some(BigDecimal::zero()),
            liquidity_locked: Some(false),
            lp_burned: Some(false),
            mint_authority_disabled: mint_data.mint_authority.is_none(),
            freeze_authority_disabled: mint_data.freeze_authority.is_none(),
            raydium_pool: None,
            pump_fun_bonding_curve: None,
            orca_pool: None,
            meteora_pool: None,
            four_meme_pool: None,
            base_pair: None,
            bsc_pair: None,
            score: None,
            risk_level: None,
        }))
    }
}