mod evm;
mod solana;
mod state;
mod xrp;

use candid::{CandidType, Principal};
use ic_cdk::{init,query, update};
use serde::{Deserialize, Serialize};
use crate::solana::TransactionRecord;
use candid::Nat;


#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct ChainConfig {
    pub chain_id: String,  // "evm", "solana", "xrp"
    pub signers: Vec<Principal>,
    pub threshold: u32,
    pub network: String,  // "mainnet", "testnet", "devnet"
}


#[update]
async fn init_multisig(configs: Vec<ChainConfig>) -> Result<String, String> {
    let mut results = Vec::new();
    
    for config in configs {
        match config.chain_id.as_str() {
            "solana" => {
                solana::init_solana_multisig(config.signers, config.threshold)?;
                results.push("Solana initialized".to_string());
            },
            "xrp" => {
                // xrp::init_chain_multisig(config.signers, config.threshold)?;
                results.push("XRP initialized".to_string());
            },
            _ => return Err(format!("Unknown chain: {}", config.chain_id))
        }
    }
    
    Ok(format!("Initialized {} chains: {}", results.len(), results.join(", ")))
}

// Single initialization for all chains with same signers
#[update]
async fn init_all_chains(signers: Vec<Principal>, threshold: u32) -> Result<String, String> {
    if threshold == 0 || threshold as usize > signers.len() {
        return Err("Invalid threshold".to_string());
    }
    
    // Initialize all chains with the same signers and threshold
    solana::init_solana_multisig(signers.clone(), threshold)?;
    
    Ok("All chains initialized with multisig".to_string())
}

#[update]
async fn create_or_sign_transaction(chain: String,to_address: String, amount: String) -> Result<String, String> {
    match chain.as_str() {
        "solana" => solana::create_or_sign_solana_transaction(to_address,amount).await,
        // "xrp" => xrp::create_or_sign_xrp_message(tx_id).await,
        _ => Err("Unsupported chain".to_string()),
    }
}


#[query]
fn get_supported_chains() -> Vec<String> {
    vec![
        "solana".to_string(),
        "xrp".to_string(),
    ]
}

#[query]
fn health_check() -> String {
    "Ordinox ECDSA multi-chain canister is running".to_string()
}

// Export candid
ic_cdk::export_candid!();