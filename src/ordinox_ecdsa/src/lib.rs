mod solana;
mod state;
mod xrp;
mod evm;
mod balance;

use candid::{CandidType, Principal};
use ic_cdk::{init,query, update};
use serde::{Deserialize, Serialize};
use crate::solana::TransactionRecord;
use ic_cdk::api::management_canister::http_request::TransformArgs;
use ic_cdk::api::management_canister::http_request::HttpResponse;
pub use xrp::{XrpKeyInfo, XrpTransactionRecord, XrpTransaction};
pub use evm::{EthKeyInfo, EthTransactionRecord, EthTransaction};
use candid::Nat;
use balance::BalanceInfo;


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
                xrp::init_xrp_multisig(config.signers, config.threshold)?;
                results.push("XRP initialized".to_string());
            },
            "evm" => {
                evm::init_eth_multisig(config.signers, config.threshold)?;
                evm::get_eth_key_info(); //Important to generate key info during initialization
                results.push("EVM initialized".to_string());
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
async fn create_or_sign_transaction(chain: String,msg_id: String,to_address: String, amount: String) -> Result<String, String> {
    match chain.as_str() {
        "solana" => solana::create_or_sign_solana_transaction(msg_id,to_address,amount).await,
        "xrp" => xrp::create_or_sign_xrp_transaction(msg_id,to_address,amount).await,
        "evm" => evm::create_or_sign_eth_transaction(msg_id,to_address,amount).await,

        _ => Err("Unsupported chain".to_string()),
    }
}


#[query]
fn get_supported_chains() -> Vec<String> {
    vec![
        "solana".to_string(),
        "xrp".to_string(),
        "evm".to_string(),
    ]
}

#[update]
async fn get_wallet_address(chain: String) -> Result<String, String> {
     match chain.as_str() {
        "solana" => solana::get_canister_solana_address().await,
        "evm" => evm::get_eth_address().await,
        _ => Err("Unsupported chain".to_string()),
    }
}

#[update]
async fn get_wallet_balance(chain: String) -> Result<String, String> {
      match chain.as_str() {
        "solana" => solana::get_wallet_balance().await,
        "evm" => evm::get_eth_balance().await,

        _ => Err("Unsupported chain".to_string()),
    }
}

#[update]
async fn check_ledger_balance() -> Result<BalanceInfo, String> {
    let caller = ic_cdk::caller();
    balance::get_balance_info(caller).await
}

#[query]
fn get_cycles() -> u128 {
    balance::get_cycles()
}


#[query]
fn health_check() -> String {
    "Ordinox ECDSA multi-chain canister is running".to_string()
}

// Export candid
ic_cdk::export_candid!();