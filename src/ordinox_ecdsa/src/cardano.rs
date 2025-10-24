use candid::{CandidType, Deserialize, Principal};
use ic_cdk::{api, caller, query, update};
use ic_cdk::api::management_canister::ecdsa::{
    ecdsa_public_key, sign_with_ecdsa, EcdsaCurve, EcdsaKeyId, 
    EcdsaPublicKeyArgument, EcdsaPublicKeyResponse, SignWithEcdsaArgument, SignWithEcdsaResponse,
};
use ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod, HttpResponse, TransformArgs,
};
use serde::{Serialize, Deserialize as SerdeDeserialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashMap;
use hex;
use bech32::{ToBase32, Variant, FromBase32};
use serde_cbor;



// ==================== Constants ====================

const ECDSA_KEY_NAME: &str = "key_1";
const BLOCKFROST_PREPROD: &str = "https://cardano-preprod.blockfrost.io/api/v0";
const BLOCKFROST_MAINNET: &str = "https://cardano-mainnet.blockfrost.io/api/v0";
const BLOCKFROST_API_KEY: &str  = "mainnetThVxLHKeXzlk3bMlYNzTRJyAYqO8zcPu";

// ==================== Types ====================

#[derive(Clone, Debug, CandidType, Serialize, Deserialize, PartialEq)]
pub enum Network {
    Mainnet,
    Preprod,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct CardanoKeyInfo {
    pub public_key: Vec<u8>,
    pub cardano_address: String,
    pub derivation_path: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct UTXO {
    pub tx_hash: String,
    pub output_index: u32,
    pub amount: u64,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub id: String,
    pub to_address: String,
    pub amount: u64,
    pub signers: Vec<Principal>,
    pub executed: bool,
    pub tx_hash: Option<String>,
    pub timestamp: u64,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct ConfigInfo {
    pub signers: Vec<Principal>,
    pub threshold: u32,
    pub network: Network,
    pub address: Option<String>,
    pub ecdsa_key: String,
}

#[derive(SerdeDeserialize)]
struct BlockfrostAddress {
    address: String,
    amount: Vec<BlockfrostAmount>,
    #[serde(default)]
    stake_address: Option<String>,
}

// ==================== State ====================

#[derive(Default)]
struct State {
    signers: Vec<Principal>,
    threshold: u32,
    transactions: HashMap<String, TransactionRecord>,
    key_info: Option<CardanoKeyInfo>,
    utxos: Vec<UTXO>,
    network: Network,
    blockfrost_api_key: String,
}

impl Default for Network {
    fn default() -> Self {
        Network::Mainnet
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

// ==================== Blockfrost Types ====================

#[derive(SerdeDeserialize)]
struct BlockfrostUTXO {
    tx_hash: String,
    output_index: u32,
    amount: Vec<BlockfrostAmount>,
}

#[derive(SerdeDeserialize)]
struct BlockfrostAmount {
    unit: String,
    quantity: String,
}

// ==================== Initialization ====================

pub  fn init_cardano_multisig(
    signers: Vec<Principal>,
    threshold: u32,
) -> Result<String, String> {
    if signers.is_empty() {
        return Err("Signers list cannot be empty".to_string());
    }
    
    if threshold == 0 || threshold > signers.len() as u32 {
        return Err(format!("Invalid threshold: must be between 1 and {}", signers.len()));
    }
    
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.signers = signers.clone();
        s.threshold = threshold;
        s.network =  Network::Mainnet;
        s.blockfrost_api_key = BLOCKFROST_API_KEY.to_string();
    });
    
    Ok(format!("Initialized: {} signers, threshold {}", signers.len(), threshold))
}

// ==================== Address Generation ====================

#[update]
pub async fn get_cardano_address() -> Result<String, String> {
    let caller_principal = caller();
    let derivation_path = vec![b"cardano".to_vec(), caller_principal.as_slice().to_vec()];
    
    let request = EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path: derivation_path.clone(),
        key_id: EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: ECDSA_KEY_NAME.to_string(),
        },
    };
    
    let (response,): (EcdsaPublicKeyResponse,) = ecdsa_public_key(request)
        .await
        .map_err(|e| format!("Failed to get public key: {:?}", e))?;
    
    let network = STATE.with(|s| s.borrow().network.clone());
    let address = derive_cardano_address(&response.public_key, &network)?;
    
    let key_info = CardanoKeyInfo {
        public_key: response.public_key,
        cardano_address: address.clone(),
        derivation_path,
    };
    
    STATE.with(|state| {
        state.borrow_mut().key_info = Some(key_info);
    });
    
    Ok(address)
}

fn derive_cardano_address(public_key: &[u8], network: &Network) -> Result<String, String> {
    // Hash the public key
    let mut hasher = Sha256::new();
    hasher.update(public_key);
    let key_hash = hasher.finalize();
    
    // Cardano address header byte
    // 0b0110_0001 = 0x61 for mainnet enterprise address
    // 0b0110_0000 = 0x60 for testnet enterprise address
    let header: u8 = match network {
        Network::Mainnet => 0x61,  // Enterprise address mainnet
        Network::Preprod => 0x60,  // Enterprise address testnet
    };
    
    // Build address bytes: [header][28-byte payment credential]
    let mut address_bytes = vec![header];
    address_bytes.extend_from_slice(&key_hash[..28]);
    
    // Bech32 human-readable part
    let hrp = match network {
        Network::Mainnet => "addr",
        Network::Preprod => "addr_test",
    };
    
    // Encode with bech32
    let address = bech32::encode(
        hrp,
        address_bytes.to_base32(),
        Variant::Bech32
    ).map_err(|e| format!("Bech32 encoding failed: {:?}", e))?;
    
    Ok(address)
}
// ==================== Balance Check ====================

#[update]
pub async fn get_balance() -> Result<String, String> {
    let (address, api_key, network) = STATE.with(|state| {
        let s = state.borrow();
        (
            s.key_info.as_ref().map(|k| k.cardano_address.clone()),
            s.blockfrost_api_key.clone(),  
            s.network.clone(),
        )
    });
    
    let address = address.ok_or("Address not generated. Call get_cardano_address first")?;
    
    if api_key.is_empty() {
        return Err("API key not set. Call init first".to_string());
    }
    
    let base_url = match network {
        Network::Mainnet => BLOCKFROST_MAINNET,
        Network::Preprod => BLOCKFROST_PREPROD,
    };
    
    let url = format!("{}/addresses/{}", base_url, address);
    
    ic_cdk::println!("🌐 Full URL: {}", url);
    ic_cdk::println!("🔑 API Key length: {}", api_key.len());
    ic_cdk::println!("🔑 API Key prefix: {}", &api_key[..10.min(api_key.len())]);
    
    let request = CanisterHttpRequestArgument {
        url,
        method: HttpMethod::GET,
        body: None,
        max_response_bytes: Some(2_000_000),
        transform: None,
        headers: vec![
            HttpHeader {
                name: "project_id".to_string(),
                value: api_key,
            },
        ],
    };
    
    let (response,): (HttpResponse,) = http_request(request, 25_000_000_000)
        .await
        .map_err(|e| format!("HTTP request failed: {:?}", e))?;
    
    ic_cdk::println!("✅ Response status: {}", response.status);
    
    if response.status != 200u64 {
        let error_body = String::from_utf8(response.body.clone())
            .unwrap_or_else(|_| "Unable to parse error".to_string());
        ic_cdk::println!("❌ Error body: {}", error_body);
        return Err(format!("Blockfrost error: status {}, body: {}", response.status, error_body));
    }
    
    let body = String::from_utf8(response.body)
        .map_err(|_| "Invalid response")?;
    
    let addr_info: BlockfrostAddress = serde_json::from_str(&body)
        .map_err(|e| format!("Parse error: {}", e))?;
    
    let total_lovelace = addr_info.amount
        .iter()
        .find(|a| a.unit == "lovelace")
        .and_then(|a| a.quantity.parse::<u64>().ok())
        .unwrap_or(0);
    
    let ada = total_lovelace as f64 / 1_000_000.0;
    Ok(format!("{:.6} ADA ({} Lovelace)", ada, total_lovelace))
}

// ==================== Create & Sign Transaction ====================

#[update]
pub async fn create_or_sign_cardano_transaction(
    msg_id: String,
    to_address: String,
    amount: String,
) -> Result<String, String> {
    let caller_principal = caller();
    
    // Validation
    if msg_id.trim().is_empty() {
        return Err("Transaction ID cannot be empty".to_string());
    }
    
    if to_address.trim().is_empty() {
        return Err("Address cannot be empty".to_string());
    }
    
    let (is_authorized, network, threshold) = STATE.with(|state| {
        let s = state.borrow();
        (
            s.signers.contains(&caller_principal),
            s.network.clone(),
            s.threshold,
        )
    });
    
    if !is_authorized {
        return Err("Caller not authorized".to_string());
    }
    
    validate_address(&to_address, &network)?;
    
    let amount_lovelace = parse_ada_amount(&amount)?;
    
    if amount_lovelace < 1_000_000 {
        return Err("Minimum amount: 1 ADA".to_string());
    }
    
    // Check if transaction exists
    let tx_exists = STATE.with(|state| {
        state.borrow().transactions.contains_key(&msg_id)
    });
    
    if tx_exists {
        // Add signature
        let should_execute = STATE.with(|state| {
            let mut s = state.borrow_mut();
            if let Some(tx) = s.transactions.get_mut(&msg_id) {
                if tx.executed {
                    return false;
                }
                if !tx.signers.contains(&caller_principal) {
                    tx.signers.push(caller_principal);
                }
                tx.signers.len() as u32 >= s.threshold
            } else {
                false
            }
        });
        
        if should_execute {
            execute_transaction(msg_id.clone()).await
        } else {
            let count = STATE.with(|s| {
                s.borrow().transactions.get(&msg_id).unwrap().signers.len()
            });
            Ok(format!("Signature added: {}/{}", count, threshold))
        }
    } else {
        // Create new transaction
        let tx = TransactionRecord {
            id: msg_id.clone(),
            to_address,
            amount: amount_lovelace,
            signers: vec![caller_principal],
            executed: false,
            tx_hash: None,
            timestamp: api::time(),
        };
        
        STATE.with(|state| {
            state.borrow_mut().transactions.insert(msg_id.clone(), tx);
        });
        
        if threshold == 1 {
            execute_transaction(msg_id.clone()).await
        } else {
            Ok(format!("Transaction created: 1/{}", threshold))
        }
    }
}

async fn execute_transaction(msg_id: String) -> Result<String, String> {
    let (tx, key_info, utxos) = STATE.with(|state| {
        let s = state.borrow();
        (
            s.transactions.get(&msg_id).cloned(),
            s.key_info.clone(),
            s.utxos.clone(),
        )
    });
    
    let tx = tx.ok_or("Transaction not found")?;
    let key_info = key_info.ok_or("Key info not initialized")?;
    
    if utxos.is_empty() {
        ic_cdk::println!("📦 No cached UTXOs, fetching from blockchain...");
        fetch_utxos_internal().await?;
    }

    // Get fresh UTXOs from state
    let utxos = STATE.with(|s| s.borrow().utxos.clone());
    
    if utxos.is_empty() {
        return Err("No UTXOs available. Fund your address first".to_string());
    }

    
    // Select UTXOs
    let fee = 200_000u64; // ~0.2 ADA
    let total_needed = tx.amount + fee;
    
    let (selected, total_input) = select_utxos(&utxos, total_needed)?;
    
    if total_input < total_needed {
        return Err(format!(
            "Insufficient funds: need {} ADA, have {} ADA",
            total_needed as f64 / 1_000_000.0,
            total_input as f64 / 1_000_000.0
        ));
    }
    
    let change = total_input - tx.amount - fee;
    
    // Build transaction hash
    let tx_body_hash = create_tx_hash(&tx, &selected, change, fee, &key_info.cardano_address)?;
    
    ic_cdk::println!("Signing transaction {}: {} ADA", msg_id, tx.amount as f64 / 1_000_000.0);
    
    // Sign with ECDSA
    let request = SignWithEcdsaArgument {
        message_hash: tx_body_hash,
        derivation_path: key_info.derivation_path.clone(),
        key_id: EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: ECDSA_KEY_NAME.to_string(),
        },
    };
    
    let (response,): (SignWithEcdsaResponse,) = sign_with_ecdsa(request)
        .await
        .map_err(|e| format!("Signing failed: {:?}", e))?;
    
    ic_cdk::println!("Signature created: {} bytes", response.signature.len());
    
    // Build and submit transaction
    let signed_tx = build_signed_tx(&tx, &selected, change, fee, &response.signature, &key_info)?;
    let tx_hash = submit_transaction(&signed_tx).await?;
    
    // Update state
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        if let Some(transaction) = s.transactions.get_mut(&msg_id) {
            transaction.executed = true;
            transaction.tx_hash = Some(tx_hash.clone());
        }
    });
    
    let explorer = get_explorer_url(&tx_hash);
    Ok(format!(
        "Transaction executed!\nTx Hash: {}\nExplorer: {}",
        tx_hash, explorer
    ))
}


async fn fetch_utxos_internal() -> Result<(), String> {
    let (address, api_key, network) = STATE.with(|state| {
        let s = state.borrow();
        (
            s.key_info.as_ref().map(|k| k.cardano_address.clone()),
            s.blockfrost_api_key.clone(),
            s.network.clone(),
        )
    });
    
    let address = address.ok_or("Address not initialized")?;
    
    if api_key.is_empty() {
        return Err("API key not set".to_string());
    }
    
    let base_url = match network {
        Network::Mainnet => BLOCKFROST_MAINNET,
        Network::Preprod => BLOCKFROST_PREPROD,
    };
    
    let url = format!("{}/addresses/{}/utxos", base_url, address);
    
    let request = CanisterHttpRequestArgument {
        url,
        method: HttpMethod::GET,
        body: None,
        max_response_bytes: Some(2_000_000),
        transform: None,
        headers: vec![
            HttpHeader {
                name: "project_id".to_string(),
                value: api_key,
            },
        ],
    };
    
    let (response,): (HttpResponse,) = http_request(request, 25_000_000_000)
        .await
        .map_err(|e| format!("HTTP request failed: {:?}", e))?;
    
    if response.status != 200u64 {
        return Err(format!("Blockfrost error: status {}", response.status));
    }
    
    let body = String::from_utf8(response.body)
        .map_err(|_| "Invalid response")?;
    
    let blockfrost_utxos: Vec<BlockfrostUTXO> = serde_json::from_str(&body)
        .map_err(|e| format!("Parse error: {}", e))?;
    
    let cached_utxos: Vec<UTXO> = blockfrost_utxos
        .into_iter()
        .filter_map(|utxo| {
            let ada = utxo.amount.iter()
                .find(|a| a.unit == "lovelace")
                .and_then(|a| a.quantity.parse::<u64>().ok())?;
            
            Some(UTXO {
                tx_hash: utxo.tx_hash,
                output_index: utxo.output_index,
                amount: ada,
            })
        })
        .collect();
    
    STATE.with(|state| {
        state.borrow_mut().utxos = cached_utxos;
    });
    
    Ok(())
}

fn select_utxos(utxos: &[UTXO], amount_needed: u64) -> Result<(Vec<UTXO>, u64), String> {
    let mut selected = Vec::new();
    let mut total = 0u64;
    
    let mut sorted = utxos.to_vec();
    sorted.sort_by(|a, b| b.amount.cmp(&a.amount));
    
    for utxo in sorted {
        selected.push(utxo.clone());
        total += utxo.amount;
        if total >= amount_needed {
            return Ok((selected, total));
        }
    }
    
    Err("Insufficient funds".to_string())
}

fn create_tx_hash(
    tx: &TransactionRecord,
    inputs: &[UTXO],
    change: u64,
    fee: u64,
    change_addr: &str,
) -> Result<Vec<u8>, String> {
    use serde_cbor::Value;
    use blake2::{Blake2b, Digest};

    // Build transaction inputs
    let tx_inputs: Vec<Value> = inputs
        .iter()
        .map(|i| {
            let tx_hash = hex::decode(&i.tx_hash)
                .map_err(|e| format!("Invalid tx_hash: {}", e))?;
            if tx_hash.len() != 32 {
                return Err("Transaction hash must be 32 bytes".to_string());
            }
            if i.output_index > 1000 {
                return Err("Output index too large".to_string());
            }
            Ok(Value::Array(vec![
                Value::Bytes(tx_hash),
                Value::Integer(i.output_index as i128),
            ]))
        })
        .collect::<Result<Vec<_>, String>>()?;

    // Build transaction outputs
    let tx_outputs: Vec<Value> = vec![
        Value::Array(vec![
            Value::Bytes(decode_address(&tx.to_address)?),
            Value::Integer(tx.amount as i128),
        ]),
        Value::Array(vec![
            Value::Bytes(decode_address(change_addr)?),
            Value::Integer(change as i128),
        ]),
    ];

    // TTL
    let current_slot = (api::time() / 1_000_000_000) as i128;
    let ttl = current_slot + 3600; // Note: May need adjustment for Cardano slots

    // Transaction body
    let body = Value::Map(vec![
        (Value::Integer(0), Value::Array(tx_inputs)),
        (Value::Integer(1), Value::Array(tx_outputs)),
        (Value::Integer(2), Value::Integer(fee as i128)),
        (Value::Integer(3), Value::Integer(ttl)),
    ].into_iter().collect());

    // Encode to CBOR
    let mut cbor_bytes = Vec::new();
    serde_cbor::to_writer(&mut cbor_bytes, &body)
        .map_err(|e| format!("CBOR encoding failed: {}", e))?;

    // Hash with Blake2b-256
    let mut hasher = Blake2b::new_with_params(&[], &[], 32); // 32-byte hash
    hasher.update(&cbor_bytes);
    Ok(hasher.finalize().to_vec())
}

fn build_signed_tx(
    tx: &TransactionRecord,
    inputs: &[UTXO],
    change: u64,
    fee: u64,
    signature: &[u8],
    key_info: &CardanoKeyInfo,
) -> Result<Vec<u8>, String> {
    use serde_cbor::Value;

    // Validate inputs
    if inputs.is_empty() {
        return Err("No transaction inputs provided".to_string());
    }
    if signature.len() != 64 {
        return Err(format!("Invalid signature length: {}", signature.len()));
    }
    if tx.amount == 0 {
        return Err("Transaction amount must be positive".to_string());
    }
    if fee == 0 {
        return Err("Fee must be positive".to_string());
    }

    // Validate input amount covers output + fee
    let total_input: u64 = inputs.iter().map(|i| i.amount).sum();
    if total_input < tx.amount + change + fee {
        return Err("Insufficient input amount".to_string());
    }

    // Build transaction inputs
    let tx_inputs: Vec<Value> = inputs
        .iter()
        .map(|i| {
            let tx_hash = hex::decode(&i.tx_hash)
                .map_err(|e| format!("Invalid tx_hash: {}", e))?;
            if tx_hash.len() != 32 {
                return Err("Transaction hash must be 32 bytes".to_string());
            }
            // Optional: Add a reasonable bounds check if needed
            if i.output_index > 1000 {
                return Err("Output index too large".to_string());
            }
            Ok(Value::Array(vec![
                Value::Bytes(tx_hash),
                Value::Integer(i.output_index as i128),
            ]))
        })
        .collect::<Result<Vec<_>, String>>()?;

    // Build transaction outputs
    let tx_outputs: Vec<Value> = vec![
        Value::Array(vec![
            Value::Bytes(decode_address(&tx.to_address)?),
            Value::Integer(tx.amount as i128),
        ]),
        Value::Array(vec![
            Value::Bytes(decode_address(&key_info.cardano_address)?),
            Value::Integer(change as i128),
        ]),
    ];

    // Process public key (ensure 32 bytes)
    let public_key_bytes = if key_info.public_key.len() == 33 && (key_info.public_key[0] == 0x02 || key_info.public_key[0] == 0x03) {
        key_info.public_key[1..33].to_vec()
    } else if key_info.public_key.len() == 65 {
        let mut hasher = Sha256::new();
        hasher.update(&key_info.public_key);
        hasher.finalize()[..32].to_vec()
    } else {
        return Err(format!("Invalid public key length: {}", key_info.public_key.len()));
    };

    // TTL
    let current_slot = (api::time() / 1_000_000_000) as i128; // Convert nanoseconds to seconds
    let ttl = current_slot + 3600; // 1 hour from now

    // Transaction body
    let body = Value::Map(vec![
        (Value::Integer(0), Value::Array(tx_inputs)),  // inputs
        (Value::Integer(1), Value::Array(tx_outputs)), // outputs
        (Value::Integer(2), Value::Integer(fee as i128)), // fee
        (Value::Integer(3), Value::Integer(ttl)),      // TTL
    ].into_iter().collect());

    // Witness set
    let vkey_witness = Value::Array(vec![
        Value::Bytes(public_key_bytes),
        Value::Bytes(signature.to_vec()),
    ]);
    let witness_set = Value::Map(vec![
        (Value::Integer(0), Value::Array(vec![vkey_witness])),
    ].into_iter().collect());

    // Transaction (use null for no auxiliary data)
    let transaction = Value::Array(vec![
        body,
        witness_set,
        Value::Null, // No auxiliary data
    ]);

    // Encode to CBOR
    let mut cbor_bytes = Vec::new();
    serde_cbor::to_writer(&mut cbor_bytes, &transaction)
        .map_err(|e| format!("CBOR encoding failed: {}", e))?;

    // Log CBOR for debugging
    ic_cdk::println!("📦 CBOR transaction size: {} bytes", cbor_bytes.len());
    ic_cdk::println!("📦 CBOR transaction hex: {}", hex::encode(&cbor_bytes));

    Ok(cbor_bytes)
}

// Helper function to decode bech32 address to bytes
fn decode_address(addr: &str) -> Result<Vec<u8>, String> {
    use bech32::FromBase32;
    
    let (_, data, _) = bech32::decode(addr)
        .map_err(|e| format!("Invalid address: {}", e))?;
    
    Vec::<u8>::from_base32(&data)
        .map_err(|e| format!("Address decode failed: {}", e))
}

async fn submit_transaction(tx_cbor: &[u8]) -> Result<String, String> {
    let (api_key, network) = STATE.with(|state| {
        let s = state.borrow();
        (s.blockfrost_api_key.clone(), s.network.clone())
    });
    
    
    let base_url = match network {
        Network::Mainnet => BLOCKFROST_MAINNET,
        Network::Preprod => BLOCKFROST_PREPROD,
    };
    
    let url = format!("{}/tx/submit", base_url);
    
    let request = CanisterHttpRequestArgument {
        url,
        method: HttpMethod::POST,
        body: Some(tx_cbor.to_vec()),
        max_response_bytes: Some(2_000_000),
        transform: None,
        headers: vec![
            HttpHeader {
                name: "project_id".to_string(),
                value: api_key,
            },
            HttpHeader {
                name: "Content-Type".to_string(),
                value: "application/cbor".to_string(),
            },
        ],
    };
    
    let (response,): (HttpResponse,) = http_request(request, 25_000_000_000)
        .await
        .map_err(|e| format!("Submission failed: {:?}", e))?;
    
    let body = String::from_utf8(response.body)
        .map_err(|_| "Invalid response")?;
    
    if response.status != 200u64 {
        return Err(format!("Submission failed: {}", body));
    }
    
    if body.starts_with('"') && body.ends_with('"') {
        Ok(body.trim_matches('"').to_string())
    } else {
        Ok(body)
    }
}

// ==================== Query Functions ====================

#[query]
pub fn get_cardano_transaction(msg_id: String) -> Result<TransactionRecord, String> {
    STATE.with(|state| {
        state.borrow()
            .transactions
            .get(&msg_id)
            .cloned()
            .ok_or("Transaction not found".to_string())
    })
}

#[query]
pub fn get_all_transactions() -> Vec<(String, TransactionRecord)> {
    STATE.with(|state| {
        state.borrow()
            .transactions
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    })
}

#[query]
pub fn get_config() -> ConfigInfo {
    STATE.with(|state| {
        let s = state.borrow();
        ConfigInfo {
            signers: s.signers.clone(),
            threshold: s.threshold,
            network: s.network.clone(),
            address: s.key_info.as_ref().map(|k| k.cardano_address.clone()),
            ecdsa_key: ECDSA_KEY_NAME.to_string(),
        }
    })
}

// ==================== Utilities ====================

fn validate_address(address: &str, network: &Network) -> Result<(), String> {
    let prefix = match network {
        Network::Mainnet => "addr1",
        Network::Preprod => "addr_test1",
    };
    
    if !address.starts_with(prefix) {
        return Err(format!("Invalid address for {:?}", network));
    }
    
    if address.len() < 50 {
        return Err("Address too short".to_string());
    }
    
    Ok(())
}

fn parse_ada_amount(amount: &str) -> Result<u64, String> {
    let ada: f64 = amount.parse()
        .map_err(|_| "Invalid amount format")?;
    
    if ada <= 0.0 {
        return Err("Amount must be positive".to_string());
    }
    
    Ok((ada * 1_000_000.0) as u64)
}

fn get_explorer_url(tx_hash: &str) -> String {
    let network = STATE.with(|s| s.borrow().network.clone());
    match network {
        Network::Mainnet => format!("https://cardanoscan.io/transaction/{}", tx_hash),
        Network::Preprod => format!("https://preprod.cardanoscan.io/transaction/{}", tx_hash),
    }
}

// ==================== Upgrade ====================

#[derive(CandidType, Deserialize)]
pub struct StableState {
    pub signers: Vec<Principal>,
    pub threshold: u32,
    pub transactions: Vec<(String, TransactionRecord)>,
    pub key_info: Option<CardanoKeyInfo>,
    pub network: Network,
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    let state = STATE.with(|s| {
        let state = s.borrow();
        StableState {
            signers: state.signers.clone(),
            threshold: state.threshold,
            transactions: state.transactions.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            key_info: state.key_info.clone(),
            network: state.network.clone(),
        }
    });
    
    ic_cdk::storage::stable_save((state,))
        .expect("Failed to save state");
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    let (stable_state,): (StableState,) = ic_cdk::storage::stable_restore()
        .expect("Failed to restore state");
    
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.signers = stable_state.signers;
        state.threshold = stable_state.threshold;
        state.transactions = stable_state.transactions.into_iter().collect();
        state.key_info = stable_state.key_info;
        state.network = stable_state.network;
    });
}

