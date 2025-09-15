// solana.rs - Complete working version
use ic_cdk::{query, update, caller, call};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use candid::{CandidType, Principal, Nat};
use hex;
use sha2::{Digest, Sha256};
use std::cell::RefCell;

// Store canister IDs - set these after deployment
thread_local! {
    static SOL_RPC_ID: RefCell<String> = RefCell::new(String::new());
    static BASIC_SOLANA_ID: RefCell<String> = RefCell::new(String::new());
}

// Transaction record structure
#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub signers: Vec<Principal>,
    pub to_address: String,
    pub lamports: u64,
    pub executed: bool,
    pub tx_id: Option<String>,
}

// State management
#[derive(Default, CandidType, Serialize, Deserialize)]
struct State {
    signers: Vec<Principal>,
    threshold: u32,
    transactions: HashMap<String, TransactionRecord>,
    canister_address: Option<String>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

// Initialize canister IDs
#[update]
pub fn set_canister_ids(sol_rpc: String, basic_solana: String) -> Result<String, String> {
    SOL_RPC_ID.with(|id| *id.borrow_mut() = sol_rpc);
    BASIC_SOLANA_ID.with(|id| *id.borrow_mut() = basic_solana);
    Ok("Canister IDs set".to_string())
}

// Initialize multisig
#[update]
pub fn init_solana_multisig(signers: Vec<Principal>, threshold: u32) -> Result<String, String> {
    if threshold == 0 || threshold as usize > signers.len() {
        return Err("Invalid threshold".to_string());
    }

    STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.signers = signers;
        s.threshold = threshold;
    });

    Ok("Solana multi-signature wallet initialized".to_string())
}

// Get canister's Solana address using basic_solana
#[update]
pub async fn get_canister_solana_address() -> Result<String, String> {
    // Check cache first
    let cached = STATE.with(|state| state.borrow().canister_address.clone());
    if let Some(address) = cached {
        return Ok(address);
    }

    let basic_solana_id = BASIC_SOLANA_ID.with(|id| id.borrow().clone());
    if basic_solana_id.is_empty() {
        return Err("Basic Solana ID not set. Call set_canister_ids first".to_string());
    }

    let basic_solana = Principal::from_text(&basic_solana_id)
        .map_err(|_| "Invalid basic_solana ID")?;
    
    // Call basic_solana's solana_account method
    let result: Result<(String,), _> = call(
        basic_solana,
        "solana_account",
        (None::<Principal>,),  // Use canister's default wallet
    ).await;

    match result {
        Ok((address,)) => {
            // Cache the address
            STATE.with(|state| {
                state.borrow_mut().canister_address = Some(address.clone());
            });
            Ok(address)
        },
        Err((code, msg)) => Err(format!("Failed to get address: {:?} - {}", code, msg))
    }
}

// Get wallet balance
#[update]
pub async fn get_wallet_balance() -> Result<Nat, String> {
    let basic_solana_id = BASIC_SOLANA_ID.with(|id| id.borrow().clone());
    if basic_solana_id.is_empty() {
        return Err("Basic Solana ID not set".to_string());
    }

    let basic_solana = Principal::from_text(&basic_solana_id)
        .map_err(|_| "Invalid basic_solana ID")?;
    
    let result: Result<(Nat,), _> = call(
        basic_solana,
        "get_balance",
        (None::<Principal>,),
    ).await;

    match result {
        Ok((balance,)) => Ok(balance),
        Err((code, msg)) => Err(format!("Failed to get balance: {:?} - {}", code, msg))
    }
}

// Request airdrop (for testing on devnet)
#[update]
pub async fn request_airdrop() -> Result<String, String> {
    let basic_solana_id = BASIC_SOLANA_ID.with(|id| id.borrow().clone());
    if basic_solana_id.is_empty() {
        return Err("Basic Solana ID not set".to_string());
    }

    let basic_solana = Principal::from_text(&basic_solana_id)
        .map_err(|_| "Invalid basic_solana ID")?;
    
    let amount = 2_000_000_000u64; // 2 SOL
    
    let result: Result<(String,), _> = call(
        basic_solana,
        "request_airdrop",
        (amount,),
    ).await;

    match result {
        Ok((sig,)) => Ok(format!("Airdrop successful: {}", sig)),
        Err((code, msg)) => Err(format!("Airdrop failed: {:?} - {}", code, msg))
    }
}

// Create or sign transaction
#[update]
pub async fn create_or_sign_solana_transaction(
    to_address: String,
    amount: String,  // Amount in SOL
) -> Result<String, String> {
    let caller = caller();

    // Convert SOL to lamports
    let lamports = sol_amount_to_lamports(&amount)?;

    // Create unique transaction ID
    let msg_id = hash_message(&to_address, lamports);

    // Check authorization and existence
    let (is_authorized, message_exists, threshold) = STATE.with(|state| {
        let s = state.borrow();
        let is_authorized = s.signers.contains(&caller);
        let message_exists = s.transactions.contains_key(&msg_id);
        (is_authorized, message_exists, s.threshold)
    });

    if !is_authorized {
        return Err("Caller is not an authorized signer".to_string());
    }

    if threshold == 1 {
        // Single-signer: execute immediately
        if message_exists {
            return Err("Transaction already executed".to_string());
        }

        STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.transactions.insert(msg_id.clone(), TransactionRecord {
                signers: vec![caller],
                to_address: to_address.clone(),
                lamports,
                executed: false,
                tx_id: None,
            });
        });

        execute_solana_transaction(msg_id.clone()).await
    } else {
        // Multi-signer case
        if message_exists {
            let should_execute = STATE.with(|state| {
                let mut s = state.borrow_mut();
                let tx = s.transactions.get_mut(&msg_id).unwrap();
                
                if tx.executed {
                    return false;
                }
                
                if !tx.signers.contains(&caller) {
                    tx.signers.push(caller);
                }
                
                tx.signers.len() as u32 >= threshold
            });

            if should_execute {
                execute_solana_transaction(msg_id.clone()).await
            } else {
                let signer_count = STATE.with(|s| 
                    s.borrow().transactions.get(&msg_id).unwrap().signers.len()
                );
                Ok(format!("Transaction {} signed. {}/{} signatures", 
                    msg_id, signer_count, threshold))
            }
        } else {
            // Create new transaction
            STATE.with(|state| {
                let mut s = state.borrow_mut();
                s.transactions.insert(msg_id.clone(), TransactionRecord {
                    signers: vec![caller],
                    to_address: to_address.clone(),
                    lamports,
                    executed: false,
                    tx_id: None,
                });
            });
            Ok(format!("Transaction {} created. 1/{} signatures", msg_id, threshold))
        }
    }
}

// Execute transaction via basic_solana
async fn execute_solana_transaction(msg_id: String) -> Result<String, String> {
    let tx_record = STATE.with(|state| 
        state.borrow().transactions.get(&msg_id).cloned()
    ).ok_or("Transaction not found")?;

    let basic_solana_id = BASIC_SOLANA_ID.with(|id| id.borrow().clone());
    if basic_solana_id.is_empty() {
        return Err("Basic Solana ID not set".to_string());
    }

    let basic_solana = Principal::from_text(&basic_solana_id)
        .map_err(|_| "Invalid basic_solana ID")?;

    // Convert lamports to Nat
    let amount_nat = Nat::from(tx_record.lamports);
    
    // Send SOL via basic_solana
    let result: Result<(String,), _> = call(
        basic_solana,
        "send_sol",
        (
            None::<Principal>,  // Use canister's default wallet
            tx_record.to_address.clone(),
            amount_nat,
        ),
    ).await;

    match result {
        Ok((tx_id,)) => {
            // Update transaction record
            STATE.with(|state| {
                let mut s = state.borrow_mut();
                if let Some(tx) = s.transactions.get_mut(&msg_id) {
                    tx.tx_id = Some(tx_id.clone());
                    tx.executed = true;
                }
            });
            Ok(format!("Transaction executed: {}", tx_id))
        },
        Err((code, msg)) => Err(format!("Failed to send SOL: {:?} - {}", code, msg))
    }
}

// Helper functions
fn sol_amount_to_lamports(amount_str: &str) -> Result<u64, String> {
    let sol_amount = amount_str.parse::<f64>()
        .map_err(|_| "Invalid SOL amount")?;
    
    if sol_amount < 0.0 {
        return Err("Amount cannot be negative".to_string());
    }
    
    Ok((sol_amount * 1_000_000_000.0) as u64)
}

fn hash_message(to_address: &str, lamports: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(to_address.as_bytes());
    hasher.update(&lamports.to_le_bytes());
    hasher.update(&ic_cdk::api::time().to_le_bytes());
    hex::encode(hasher.finalize())
}

// Query functions
#[query]
pub fn get_transaction(msg_id: String) -> Option<TransactionRecord> {
    STATE.with(|state| 
        state.borrow().transactions.get(&msg_id).cloned()
    )
}

#[query]
pub fn get_pending_transactions() -> Vec<(String, TransactionRecord)> {
    STATE.with(|state| {
        state.borrow()
            .transactions
            .iter()
            .filter(|(_, tx)| !tx.executed)
            .map(|(id, tx)| (id.clone(), tx.clone()))
            .collect()
    })
}

#[query]
pub fn get_executed_transactions() -> Vec<(String, TransactionRecord)> {
    STATE.with(|state| {
        state.borrow()
            .transactions
            .iter()
            .filter(|(_, tx)| tx.executed)
            .map(|(id, tx)| (id.clone(), tx.clone()))
            .collect()
    })
}

#[query]
pub fn get_signers_and_threshold() -> (Vec<Principal>, u32) {
    STATE.with(|state| {
        let s = state.borrow();
        (s.signers.clone(), s.threshold)
    })
}

#[query]
pub fn get_transaction_signers(msg_id: String) -> Vec<Principal> {
    STATE.with(|state| {
        state.borrow()
            .transactions
            .get(&msg_id)
            .map(|tx| tx.signers.clone())
            .unwrap_or_default()
    })
}