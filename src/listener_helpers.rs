use std::sync::Arc;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcTransactionConfig;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::signature::Signature;
use solana_transaction_status::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};
use tokio::sync::Semaphore;
use tracing::warn;

// pub async fn fetch_transaction_with_retry(
//     rpc: &RpcClient,
//     sig: &Signature,
// ) -> anyhow::Result<Option<EncodedConfirmedTransactionWithStatusMeta>> {
//     // First attempt: default config (no max_supported_transaction_version)
//     let default_config = RpcTransactionConfig {
//         encoding: Some(UiTransactionEncoding::JsonParsed),
//         max_supported_transaction_version: None,
//         commitment: Some(CommitmentConfig::confirmed()),
//     };
//
//     match rpc.get_transaction_with_config(sig, default_config.clone()) {
//         Ok(tx) => return Ok(Some(tx)), // Success
//         Err(err) => {
//             let err_msg = err.to_string();
//             // Check if RPC complains about version
//             if err_msg.contains("Transaction version") || err_msg.contains("not supported by the requesting client") {
//                 // Retry with version 0
//                 let retry_config = RpcTransactionConfig {
//                     encoding: Some(UiTransactionEncoding::JsonParsed),
//                     max_supported_transaction_version: Some(0),
//                     commitment: Some(CommitmentConfig::confirmed()),
//                     ..RpcTransactionConfig::default()
//                 };
//
//                 match rpc.get_transaction_with_config(sig, retry_config) {
//                     Ok(tx) => return Ok(Some(tx)),
//                     Err(retry_err) => {
//                         let retry_msg = retry_err.to_string();
//                         if retry_msg.contains("Transaction version") || retry_msg.contains("not supported by the requesting client") {
//                             // Endpoint doesn't support version 0 at all
//                             warn!(
//                                 "RPC endpoint does not support version 0 for transaction {}. Skipping fetch.",
//                                 sig
//                             );
//                             return Ok(None);
//                         } else {
//                             // Other retry errors (e.g., network)
//                             warn!("Retry failed for transaction {}: {}", sig, retry_err);
//                             return Ok(None);
//                         }
//                     }
//                 }
//             } else if err_msg.contains("invalid type: null") {
//                 // Transaction not yet finalized / null response
//                 warn!("Transaction {} not found or not yet finalized", sig);
//                 return Ok(None);
//             } else {
//                 // Other errors
//                 warn!("Failed to fetch transaction {}: {}", sig, err);
//                 return Ok(None);
//             }
//         }
//     }

pub async fn fetch_transaction_with_retry(
    rpc: &RpcClient,
    sig: &Signature,
    limiter: Arc<Semaphore>,
) -> anyhow::Result<Option<EncodedConfirmedTransactionWithStatusMeta>> {
    // Acquire 1 permit — this limits concurrent RPC calls.
    let _permit = limiter.acquire_owned().await?;

    // Helper to detect version mismatch
    let is_version_error = |msg: &str| {
        msg.contains("Transaction version")
            || msg.contains("not supported by the requesting client")
    };

    // ---- Attempt 1: default config
    let attempt_default = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::JsonParsed),
        max_supported_transaction_version: None,
        commitment: Some(CommitmentConfig::confirmed()),
    };

    match rpc.get_transaction_with_config(sig, attempt_default.clone()) {
        Ok(tx) => return Ok(Some(tx)),
        Err(err) => {
            let msg = err.to_string();
            if !is_version_error(&msg) {
                if msg.contains("invalid type: null") {
                    warn!("Transaction {} not found or not yet finalized", sig);
                } else {
                    warn!("Failed to fetch transaction {}: {}", sig, msg);
                }
                return Ok(None);
            }
        }
    }

    // ---- Attempt 2: allow version 0
    let attempt_v0 = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::JsonParsed),
        max_supported_transaction_version: Some(0),
        commitment: Some(CommitmentConfig::confirmed()),
        ..Default::default()
    };

    match rpc.get_transaction_with_config(sig, attempt_v0) {
        Ok(tx) => return Ok(Some(tx)),
        Err(err) => {
            let msg = err.to_string();
            if !is_version_error(&msg) {
                warn!("Retry (v0) failed for transaction {}: {}", sig, msg);
                return Ok(None);
            }
        }
    }

    // ---- Attempt 3: fallback None explicitly
    let attempt_none = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::JsonParsed),
        max_supported_transaction_version: None,
        commitment: Some(CommitmentConfig::confirmed()),
    };

    match rpc.get_transaction_with_config(sig, attempt_none) {
        Ok(tx) => Ok(Some(tx)),
        Err(err) => {
            warn!(
                "RPC endpoint does not support versioned fetch for {}. Error: {}",
                sig, err
            );
            Ok(None)
        }
    }


}