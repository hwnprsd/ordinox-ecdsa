use candid::{CandidType, Deserialize, Principal};
use ic_cdk::api::canister_balance128;
use ic_cdk::call;
use serde::Serialize;
use sha2::{Digest, Sha224};

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct BalanceInfo {
    pub cycles: u128,
    pub icp_e8s: u64,
    pub account_id: String,
}

#[derive(CandidType, Deserialize)]
struct AccountBalanceArgs {
    account: Vec<u8>,
}

#[derive(CandidType, Deserialize)]
struct Tokens {
    e8s: u64,
}

const LEDGER_CANISTER_ID: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";
const DOMAIN_SEPARATOR: &[u8] = b"\x0Aaccount-id";

fn principal_to_account_identifier(principal: Principal, subaccount: Option<[u8; 32]>) -> Vec<u8> {
    let mut hasher = Sha224::new();
    hasher.update(DOMAIN_SEPARATOR);
    hasher.update(principal.as_slice());
    hasher.update(subaccount.unwrap_or([0u8; 32]));
    hasher.finalize().to_vec()
}

fn account_id_to_hex(account_id: &[u8]) -> String {
    hex::encode(account_id)
}

fn hex_to_account_id(hex: &str) -> Result<Vec<u8>, String> {
    hex::decode(hex).map_err(|e| format!("Invalid hex string: {:?}", e))
}

async fn query_ledger_balance(account_id: Vec<u8>) -> Result<u64, String> {
    let ledger_principal = Principal::from_text(LEDGER_CANISTER_ID)
        .map_err(|e| format!("Invalid ledger principal: {:?}", e))?;
    
    let args = AccountBalanceArgs { account: account_id };
    
    let result: Result<(Tokens,), _> = call(
        ledger_principal,
        "account_balance",
        (args,)
    ).await;
    
    match result {
        Ok((tokens,)) => Ok(tokens.e8s),
        Err(e) => Err(format!("Failed to get ICP balance: {:?}", e))
    }
}

/// Get cycles balance
pub fn get_cycles() -> u128 {
    canister_balance128()
}

/// Get account identifier for a principal
pub fn get_account_id(principal: Principal) -> String {
    let account_id = principal_to_account_identifier(principal, None);
    account_id_to_hex(&account_id)
}

/// Get ICP balance for an account
pub async fn get_icp_balance(account_id_hex: &str) -> Result<u64, String> {
    let account_id = hex_to_account_id(account_id_hex)?;
    query_ledger_balance(account_id).await
}

/// Get complete balance info for a principal
pub async fn get_balance_info(principal: Principal) -> Result<BalanceInfo, String> {
    let account_id = principal_to_account_identifier(principal, None);
    let account_id_hex = account_id_to_hex(&account_id);
    
    let cycles = canister_balance128();
    let icp_e8s = query_ledger_balance(account_id).await?;
    
    Ok(BalanceInfo {
        cycles,
        icp_e8s,
        account_id: account_id_hex,
    })
}

/// Get both cycles and ICP balance for a specific account ID (hex string)
pub async fn get_balance(account_id_hex: String) -> Result<BalanceInfo, String> {
    let cycles = canister_balance128();
    let account_id = hex_to_account_id(&account_id_hex)?;
    let icp_e8s = query_ledger_balance(account_id).await?;
    
    Ok(BalanceInfo {
        cycles,
        icp_e8s,
        account_id: account_id_hex,
    })
}