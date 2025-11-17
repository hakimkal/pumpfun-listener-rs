use std::fmt;
use std::str::FromStr;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
 use solana_program::pubkey::Pubkey;

use serde::{Deserialize, Serialize};

use thiserror::Error;
use tokio::signal::unix::Signal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub mint_address: String,
    pub created_at: DateTime<Utc>,
    pub discovered_at: DateTime<Utc>,
    pub source: TokenSource,

    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: u8,

    pub total_supply: BigDecimal,
    pub holder_count: Option<u32>,
    pub top_10_holder_percentage: Option<BigDecimal>,

    pub liquidity_sol: Option<BigDecimal>,
    pub liquidity_locked: Option<bool>,
    pub lp_burned: Option<bool>,

    pub mint_authority_disabled: bool,
    pub freeze_authority_disabled: bool,

    pub raydium_pool: Option<Pubkey>,
    pub pump_fun_bonding_curve: Option<Pubkey>,
    pub orca_pool: Option<String>,
    pub meteora_pool: Option<String>,
    pub four_meme_pool: Option<String>,
    pub base_pair: Option<String>,
    pub bsc_pair: Option<String>,
    pub score: Option<i32>,
    pub risk_level: Option<RiskLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}
impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                RiskLevel::Low => "Low",
                RiskLevel::Medium => "Medium",
                RiskLevel::High => "High",
            }
        )
    }
}


#[derive(Debug, Clone, Serialize, Deserialize,PartialEq,Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TokenSource {

    Pumpfun,


    OnChain,
}
impl fmt::Display for TokenSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TokenSource::OnChain => "Onchain",

                TokenSource::Pumpfun => "Pumpfun",

            }
        )
    }
}

impl FromStr for TokenSource {
    type Err = TokenSourceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
             "pumpfun" => Ok(TokenSource::Pumpfun),

            "onchain" | "on-chain" => Ok(TokenSource::OnChain),
            _ => Err(TokenSourceParseError::InvalidSource(s.to_string())),
        }
    }
}

#[derive(Error, Debug)]
pub enum TokenSourceParseError {
    #[error("Invalid token source: {0}")]
    InvalidSource(String),
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    TokenDiscovered(Token),
    
}