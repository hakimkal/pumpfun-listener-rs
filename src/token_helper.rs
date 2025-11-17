use std::str::FromStr;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::json;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcTransactionConfig;
use solana_sdk::pubkey::Pubkey;
use solana_program::program_option::COption;
use spl_token::solana_program::program_pack::Pack;
// Legacy SPL Token
use spl_token::state::Mint as LegacyMint;

// SPL Token-2022
use spl_token_2022::state::Mint as Mint2022;
use tracing::{info, warn};

#[derive(Debug)]
pub enum MintProgramType {
    Token,
    Token2022,
}

#[derive(Debug)]
pub struct MintInfo {
    pub program: MintProgramType,
    pub decimals: u8,
    pub supply: u64,
    pub mint_authority: COption<Pubkey>,
    pub freeze_authority: COption<Pubkey>,
}

/// Load and parse a mint account from chain safely
pub fn load_mint_info(rpc: &RpcClient, mint: &Pubkey) -> anyhow::Result<Option<MintInfo>> {
    let account = match rpc.get_account(mint) {
        Ok(acc) => acc,
        Err(e) => {
            warn!("Failed to load mint account {}: {:?}", mint, e);
            return Ok(None);
        }
    };

    info!("Solana Account in load mint helper {:?}", account);

    if account.owner == spl_token::ID {
        let mint_data = LegacyMint::unpack(&account.data);
        match mint_data {
            Ok(mint) => Ok(Some(MintInfo {
                program: MintProgramType::Token,
                decimals: mint.decimals,
                supply: mint.supply,
                mint_authority: mint.mint_authority,
                freeze_authority: mint.freeze_authority,
            })),
            Err(e) => {
                warn!("Failed to unpack legacy SPL mint {}: {:?}", mint, e);
                Ok(None)
            }
        }
    } else if account.owner == spl_token_2022::ID {
        let mint_data = spl_token_2022::state::Mint::unpack(&account.data);
        match mint_data {
            Ok(mint) => Ok(Some(MintInfo {
                program: MintProgramType::Token2022,
                decimals: mint.decimals,
                supply: mint.supply,
                mint_authority: mint.mint_authority,
                freeze_authority: mint.freeze_authority,
            })),
            Err(e) => {
                warn!("Failed to unpack SPL-2022 mint {}: {:?}", mint, e);
                Ok(None)
            }
        }
    } else {
        warn!("Account {} is not a valid SPL token mint (custom program)", mint);
        Ok(None)
    }
}


/// Helper to parse legacy SPL Token Mint manually
fn parse_spl_token_mint(data: &[u8]) -> Result<LegacyMint> {
    let mint = LegacyMint::unpack(data)
        .map_err(|e| anyhow!("Failed to unpack legacy SPL mint: {}", e))?;
    Ok(mint)
}

/// Helper to parse SPL-2022 Mint manually
fn parse_spl_token_2022_mint(data: &[u8]) -> Result<spl_token_2022::state::Mint> {
    let mint = spl_token_2022::state::Mint::unpack(data)
        .map_err(|e| anyhow!("Failed to unpack SPL-2022 mint: {}", e))?;
    Ok(mint)
}


#[derive(Debug)]
pub struct TokenInfo {
    pub name: String,
    pub symbol: String,
}

pub async fn fetch_token_info(
listener:&str,
    mint_address: &str,
    chain_id: &str, // "solana" for DexScreener
) -> Result<TokenInfo> {


    let client = Client::new();

    let mut name = "Unknown".to_string();
    let mut symbol = "UNK".to_string();



    // 3️⃣ Fallback to DexScreener
    if name == "Unknown" {
        let dex_url = format!(
            "https://api.dexscreener.com/tokens/v1/{}/{}",
            chain_id, mint_address
        );
        info!("Fetching dex url: {} for listener: {}", dex_url ,listener);

        if let Ok(resp) = client.get(&dex_url).send().await {
            if resp.status().is_success() {

                let respo_text=resp.text().await?;
                // info!("Dex API response: {:?}", &respo_text);
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&respo_text) {
                    if let Some(arr) = json.as_array() {
                        if let Some(pair) = arr.first() {
                            name = pair
                                .get("baseToken")
                                .and_then(|t| t.get("name"))
                                .and_then(|v| v.as_str())
                                .unwrap_or(&name)
                                .to_string();

                            symbol = pair
                                .get("baseToken")
                                .and_then(|t| t.get("symbol"))
                                .and_then(|v| v.as_str())
                                .unwrap_or(&symbol)
                                .to_string();
                        }
                    }
                }
            }
        }
    }

    info!("Token info: {} {}", &name,&symbol);
    Ok(TokenInfo {

        name,
        symbol,
    })
}