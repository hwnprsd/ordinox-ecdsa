use ic_cdk::api::{self, call::call, caller};
use ic_cdk::api::management_canister::http_request::{http_request, CanisterHttpRequestArgument, HttpMethod, HttpHeader, HttpResponse, TransformContext};
use ic_cdk_macros::{update, query, pre_upgrade, post_upgrade};
use serde::{Deserialize, Serialize};
use serde_cbor::Value;
use blake2::{Blake2b, Digest};
use bech32::{self, ToBase32, Variant, FromBase32};
use std::collections::BTreeMap;
use std::cell::RefCell;
use candid::{CandidType, Principal};
use serde_json::from_str;
use ic_cdk::api::call::call_with_payment128;


thread_local! {
    static STATE: RefCell<StableState> = RefCell::new(StableState::default());
}

const SCHNORR_KEY_NAME: &str = "key_1";
const BLOCKFROST_PREPROD: &str = "https://cardano-preprod.blockfrost.io/api/v0";
const BLOCKFROST_MAINNET: &str = "https://cardano-mainnet.blockfrost.io/api/v0";


const CYCLES_FOR_SCHNORR_PUBLIC_KEY: u128 = 10_000_000_000; // 10 billion cycles
const CYCLES_FOR_SIGN_WITH_SCHNORR: u128 = 27_000_000_000; // 20 billion cycles


const LOVELACE_PER_ADA: u64 = 1_000_000;

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
struct StableState {
    signers: Vec<Principal>,
    threshold: u32,
    transactions: BTreeMap<String, TransactionRecord>,
    key_info: Option<CardanoKeyInfo>,
    network: Network,
    blockfrost_api_key: String,
    utxos: Vec<UTXO>,
    pub min_fee_a: u64, // Add to store protocol parameters
    pub min_fee_b: u64,
    pub min_utxo: u64,
}


impl Default for StableState {
    fn default() -> Self {
        StableState {
            signers: Vec::new(),
            threshold: 0,
            transactions: BTreeMap::new(),
            key_info: None,
            network: Network::Mainnet, // Default to Mainnet
            blockfrost_api_key: String::new(),
            utxos: Vec::new(),
            min_fee_a: 44, // Default fallback
            min_fee_b: 155381,
            min_utxo: 999978,
        }
    }
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
struct CardanoKeyInfo {
    ed25519_public_key: Vec<u8>,
    cardano_address: String, // Blake2b-224, Ed25519-based
    derivation_path: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
struct TransactionRecord {
    id: String,
    to_address: String,
    amount: u64, // Lovelace
    signers: Vec<Principal>,
    executed: bool,
    tx_hash: Option<String>,
    timestamp: u64,
}


#[derive(Deserialize)]
struct ProtocolParameters {
    min_fee_a: u64,
    min_fee_b: u64,
    min_utxo: u64,
}


#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub enum Network {
    Preprod,
    Mainnet,
}


#[derive(Clone, Debug, CandidType, Serialize, Deserialize,PartialEq)]
struct UTXO {
    tx_hash: String,
    output_index: u32,
    amount: u64,
    address: String,
}

#[derive(Deserialize, Serialize)]
pub struct BlockfrostAddress {
    pub address: String,
    pub amount: Vec<BlockfrostAmount>,
    #[serde(default)]
    pub stake_address: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>, // Changed from `type` to `r#type`
}
#[derive(Deserialize, Serialize)]
struct BlockfrostUTXO {
    tx_hash: String,
    output_index: u32,
    amount: Vec<BlockfrostAmount>,
    address: String,
}

#[derive(Deserialize, Serialize)]
struct BlockfrostAmount {
    unit: String,
    quantity: String,
}

#[derive(CandidType, Serialize, Deserialize)]
pub enum SchnorrAlgorithm {
    #[serde(rename = "ed25519")]
    Ed25519,
    #[serde(rename = "bip340secp256k1")]
    Bip340Secp256k1,
}
#[derive(CandidType, Serialize, Deserialize)]
struct SchnorrKeyId {
    algorithm: SchnorrAlgorithm,
    name: String,
}

#[derive(CandidType, Serialize, Deserialize)]
struct SchnorrPublicKeyArgument {
    canister_id: Option<Principal>,
    derivation_path: Vec<Vec<u8>>,
    key_id: SchnorrKeyId,
}

#[derive(CandidType, Serialize, Deserialize)]
struct SchnorrPublicKeyResponse {
    public_key: Vec<u8>,
}

#[derive(CandidType, Serialize, Deserialize)]
struct SignWithSchnorrArgument {
    message: Vec<u8>,
    derivation_path: Vec<Vec<u8>>,
    key_id: SchnorrKeyId,
}

#[derive(CandidType, Serialize, Deserialize)]
struct SignWithSchnorrResponse {
    signature: Vec<u8>,
}

#[update]
pub fn init_cardano_multisig(signers: Vec<Principal>, threshold: u32) -> Result<String, String> {
    if signers.is_empty() {
        return Err("Signers list cannot be empty".to_string());
    }
    if threshold == 0 || threshold > signers.len() as u32 {
        return Err(format!("Invalid threshold: must be between 1 and {}", signers.len()));
    }

 // Set protocol parameters based on network
    let (min_fee_a, min_fee_b, min_utxo) = match Network::Mainnet {
        Network::Preprod => (44, 155381, 431976),
        Network::Mainnet => (44, 155381, 999978),
    };

    STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.signers = signers.clone();
        s.threshold = threshold;
        s.network = Network::Mainnet;
        s.blockfrost_api_key = "mainnetThVxLHKeXzlk3bMlYNzTRJyAYqO8zcPu".to_string();
        s.min_fee_a = min_fee_a;
        s.min_fee_b = min_fee_b;
        s.min_utxo = min_utxo;
    });

    Ok(format!("Initialized: {} signers, threshold {}", signers.len(), threshold))
}

#[update]
pub async fn get_cardano_address() -> Result<String, String> {
    let caller_principal = caller();
    let derivation_path = vec![b"cardano".to_vec(), caller_principal.as_slice().to_vec()];
    
    let request = SchnorrPublicKeyArgument {
        canister_id: None,
        derivation_path: derivation_path.clone(),
        key_id: SchnorrKeyId {
            algorithm: SchnorrAlgorithm::Ed25519,
            name: SCHNORR_KEY_NAME.to_string(),
        },
    };
    

    let (response,): (SchnorrPublicKeyResponse,) = call_with_payment128(
        Principal::management_canister(),
        "schnorr_public_key",
        (request,),
        CYCLES_FOR_SCHNORR_PUBLIC_KEY,
    )
    .await
    .map_err(|e| format!("Failed to get public key: {:?}", e))?;

    let ed25519_pub_key = response.public_key;
    if ed25519_pub_key.len() != 32 {
        return Err(format!("Invalid Ed25519 public key length: {}", ed25519_pub_key.len()));
    }

    let network = STATE.with(|s| s.borrow().network.clone());
    
    let (header, hrp) = match network {
        Network::Mainnet => (0x61, "addr"),
        Network::Preprod => (0x60, "addr_test"),
    };
    
    // Generate Cardano address (Blake2b-224)
    let mut blake_hasher = Blake2b::<blake2::digest::consts::U28>::new();
    blake_hasher.update(&ed25519_pub_key);
    let blake_key_hash = blake_hasher.finalize();
    let mut address_bytes = vec![header];
    address_bytes.extend_from_slice(&blake_key_hash);
    let address = bech32::encode(
        hrp,
        address_bytes.to_base32(),
        Variant::Bech32
    ).map_err(|e| format!("Bech32 encoding failed for Cardano address: {:?}", e))?;

    let key_info = CardanoKeyInfo {
        ed25519_public_key: ed25519_pub_key,
        cardano_address: address.clone(),
        derivation_path,
    };
    
    STATE.with(|state| {
        state.borrow_mut().key_info = Some(key_info);
    });
    
    Ok(address)
}

#[query]
fn get_current_address() -> Result<String, String> {
    STATE.with(|state| {
        state
            .borrow()
            .key_info
            .as_ref()
            .map(|k| k.cardano_address.clone())
            .ok_or("Address not initialized".to_string())
    })
}



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
        return Err("API key not set. Call init_cardano_multisig first".to_string());
    }
    
    let base_url = match network {
        Network::Mainnet => BLOCKFROST_MAINNET,
        Network::Preprod => BLOCKFROST_PREPROD,
    };
    
    let url = format!("{}/addresses/{}", base_url, address);
    
    ic_cdk::println!("🌐 Full URL: {}", url);
    ic_cdk::println!("🔑 API Key length: {}", api_key.len());
    
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
        let error_body = String::from_utf8_lossy(&response.body);
        ic_cdk::println!("❌ Error body: {}", error_body);
        return Err(format!("Blockfrost error: status {}, body: {}", response.status, error_body));
    }
    
    let body = String::from_utf8(response.body)
        .map_err(|_| "Invalid response")?;
    
    let addr_info: BlockfrostAddress = from_str(&body)
        .map_err(|e| format!("Parse error: {}", e))?;
    
    let total_lovelace = addr_info.amount
        .iter()
        .find(|a| a.unit == "lovelace")
        .and_then(|a| a.quantity.parse::<u64>().ok())
        .unwrap_or(0);
    
    let ada = total_lovelace as f64 / 1_000_000.0;
    Ok(format!("{:.6} ADA ({} Lovelace)", ada, total_lovelace))
}


fn fetch_utxos_internal() -> Result<(), String> {
    let (address, min_utxo) = STATE.with(|state| {
        let s = state.borrow();
        (
            s.key_info.as_ref().map(|k| k.cardano_address.clone()),
            s.min_utxo,
        )
    });

    let address = address.ok_or("Address not initialized")?;

    // Use cached UTXOs from state (populated by update_utxos)
    let cached_utxos = STATE.with(|state| {
        state
            .borrow()
            .utxos
            .iter()
            .filter(|utxo| {
                if utxo.amount < min_utxo {
                    ic_cdk::println!(
                        "Skipping UTXO {}:{} with amount {} (below min {})",
                        utxo.tx_hash,
                        utxo.output_index,
                        utxo.amount,
                        min_utxo
                    );
                    false
                } else {
                    true
                }
            })
            .cloned()
            .collect::<Vec<UTXO>>()
    });

    if cached_utxos.is_empty() {
        ic_cdk::println!("No valid UTXOs in cache. Call update_utxos to fetch from Blockfrost.");
        return Err("No valid UTXOs available".to_string());
    }

    ic_cdk::println!("Using cached UTXOs: {:?}", cached_utxos);
    STATE.with(|state| {
        state.borrow_mut().utxos = cached_utxos;
    });

    Ok(())
}

#[update]
async fn update_utxos() -> Result<(), String> {
    let (address, api_key, network, min_utxo) = STATE.with(|state| {
        let s = state.borrow();
        (
            s.key_info.as_ref().map(|k| k.cardano_address.clone()),
            s.blockfrost_api_key.clone(),
            s.network.clone(),
            s.min_utxo,
        )
    });

    let address = address.ok_or("Address not initialized")?;

    let url = format!(
        "{}/addresses/{}/utxos",
        match network {
            Network::Mainnet => BLOCKFROST_MAINNET,
            Network::Preprod => BLOCKFROST_PREPROD,
        },
        address
    );

    let request = CanisterHttpRequestArgument {
        url,
        method: HttpMethod::GET,
        body: None,
        max_response_bytes: Some(2_000_000),
        transform: None,
        headers: vec![HttpHeader {
            name: "project_id".to_string(),
            value: api_key,
        }],
    };

    let (response,): (HttpResponse,) = http_request(request, 25_000_000_000)
        .await
        .map_err(|e| format!("HTTP request failed: {:?}", e))?;

    ic_cdk::println!("✅ UTXO Response status: {}", response.status);
    ic_cdk::println!("📄 UTXO Raw response body: {}", String::from_utf8_lossy(&response.body));

    if response.status != 200u64 {
        ic_cdk::println!("Failed to fetch UTXOs for {}: status {}", address, response.status);
        return Err(format!("Blockfrost API returned status {}", response.status));
    }

    let body = String::from_utf8(response.body).map_err(|_| "Invalid response")?;

    let blockfrost_utxos: Vec<BlockfrostUTXO> = from_str(&body).map_err(|e| format!("Parse error: {}", e))?;

    let cached_utxos: Vec<UTXO> = blockfrost_utxos
        .into_iter()
        .filter_map(|utxo| {
            let ada = utxo
                .amount
                .iter()
                .find(|a| a.unit == "lovelace")
                .and_then(|a| a.quantity.parse::<u64>().ok())?;
            if ada < min_utxo {
                ic_cdk::println!(
                    "Skipping UTXO {}:{} with amount {} (below min {})",
                    utxo.tx_hash,
                    utxo.output_index,
                    ada,
                    min_utxo
                );
                return None;
            }
            Some(UTXO {
                tx_hash: utxo.tx_hash,
                output_index: utxo.output_index,
                amount: ada,
                address: utxo.address,
            })
        })
        .collect();

    ic_cdk::println!("Fetched UTXOs: {:?}", cached_utxos);
    STATE.with(|state| {
        state.borrow_mut().utxos = cached_utxos;
    });

    Ok(())
}

fn estimate_fee(tx_size: usize) -> u64 {
    STATE.with(|state| {
        let s = state.borrow();
        s.min_fee_a * tx_size as u64 + s.min_fee_b
    })
}

fn create_tx_hash(
    tx: &TransactionRecord,
    inputs: &[UTXO],
    change: u64,
    fee: u64,
    change_addr: &str,
) -> Result<Vec<u8>, String> {
    let state = STATE.with(|s| s.borrow().clone());
    let tx_inputs: Vec<Value> = inputs
        .iter()
        .map(|i| {
            let tx_hash = hex::decode(&i.tx_hash).map_err(|e| format!("Invalid tx_hash: {}", e))?;
            Ok(Value::Array(vec![
                Value::Bytes(tx_hash),
                Value::Integer(i.output_index as i128),
            ]))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let total_input: u64 = inputs.iter().map(|i| i.amount).sum();
    let adjusted_fee = if change > 0 && change < state.min_utxo {
        fee + change
    } else {
        fee
    };

    let total_output = tx.amount + adjusted_fee + if change >= state.min_utxo { change } else { 0 };
    if total_input != total_output {
        return Err(format!(
            "Transaction not balanced: inputs={} lovelace, outputs={} lovelace (amount={}, fee={}, change={})",
            total_input, total_output, tx.amount, adjusted_fee, change
        ));
    }

    let tx_outputs: Vec<Value> = if change >= state.min_utxo {
        vec![
            Value::Array(vec![
                Value::Bytes(decode_address(&tx.to_address)?),
                Value::Integer(tx.amount as i128),
            ]),
            Value::Array(vec![
                Value::Bytes(decode_address(change_addr)?),
                Value::Integer(change as i128),
            ]),
        ]
    } else {
        vec![Value::Array(vec![
            Value::Bytes(decode_address(&tx.to_address)?),
            Value::Integer(tx.amount as i128),
        ])]
    };

    ic_cdk::println!(
        "Inputs sum: {} lovelace, Outputs sum: {} lovelace (amount: {}, fee: {}, change: {})",
        total_input, total_output, tx.amount, adjusted_fee, change
    );

    let current_slot = (api::time() / 1_000_000_000) as i128;
    let ttl = current_slot + 3600;

    let body = Value::Map(
        vec![
            (Value::Integer(0), Value::Array(tx_inputs)),
            (Value::Integer(1), Value::Array(tx_outputs)),
            (Value::Integer(2), Value::Integer(adjusted_fee as i128)),
            (Value::Integer(3), Value::Integer(ttl)),
        ]
        .into_iter()
        .collect(),
    );

    let mut cbor_bytes = Vec::new();
    serde_cbor::to_writer(&mut cbor_bytes, &body).map_err(|e| format!("CBOR encoding failed: {}", e))?;

    let mut hasher = sha2::Sha256::new();
    hasher.update(&cbor_bytes);
    Ok(hasher.finalize().to_vec())
}

async fn build_signed_tx(
    tx: &TransactionRecord,
    inputs: &[UTXO],
    change: u64,
    fee: u64,
    key_info: &CardanoKeyInfo,
) -> Result<Vec<u8>, String> {
    let state = STATE.with(|s| s.borrow().clone());
    let pub_key_bytes = key_info.ed25519_public_key.clone();

    let mut blake_hasher = Blake2b::<blake2::digest::consts::U28>::new();
    blake_hasher.update(&pub_key_bytes);
    let key_hash = blake_hasher.finalize();
    for utxo in inputs {
        let addr_bytes = decode_address(&utxo.address)?;
        if addr_bytes.len() < 29 || addr_bytes[1..29] != *key_hash.as_slice() {
            return Err(format!("UTXO address mismatch: {}", utxo.address));
        }
    }

    let total_input: u64 = inputs.iter().map(|i| i.amount).sum();
    let adjusted_fee = if change > 0 && change < state.min_utxo {
        fee + change // Absorb small change into fee
    } else {
        fee
    };

    let total_output = tx.amount + adjusted_fee + if change >= state.min_utxo { change } else { 0 };
    if total_input != total_output {
        return Err(format!(
            "Transaction not balanced: inputs={} lovelace, outputs={} lovelace (amount={}, fee={}, change={})",
            total_input, total_output, tx.amount, adjusted_fee, change
        ));
    }

    ic_cdk::println!("Transaction inputs: {:?}", inputs);
    ic_cdk::println!("Transaction amount: {} lovelace, fee: {}, adjusted_fee: {}, change: {}", 
        tx.amount, fee, adjusted_fee, change);

    let tx_inputs: Vec<Value> = inputs
        .iter()
        .map(|i| {
            let tx_hash = hex::decode(&i.tx_hash)
                .map_err(|e| format!("Invalid tx_hash: {}", e))?;
            Ok(Value::Array(vec![
                Value::Bytes(tx_hash),
                Value::Integer(i.output_index as i128),
            ]))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let tx_outputs: Vec<Value> = if change >= state.min_utxo {
        vec![
            Value::Array(vec![
                Value::Bytes(decode_address(&tx.to_address)?),
                Value::Integer(tx.amount as i128),
            ]),
            Value::Array(vec![
                Value::Bytes(decode_address(&key_info.cardano_address)?),
                Value::Integer(change as i128),
            ]),
        ]
    } else {
        vec![Value::Array(vec![
            Value::Bytes(decode_address(&tx.to_address)?),
            Value::Integer(tx.amount as i128),
        ])]
    };

    ic_cdk::println!("Transaction outputs: {:?}", tx_outputs);

    let current_slot = (api::time() / 1_000_000_000) as i128;
    let ttl = current_slot + 3600;

    let body = Value::Map(vec![
        (Value::Integer(0), Value::Array(tx_inputs)),
        (Value::Integer(1), Value::Array(tx_outputs)),
        (Value::Integer(2), Value::Integer(adjusted_fee as i128)),
        (Value::Integer(3), Value::Integer(ttl)),
    ].into_iter().collect());

    let mut cbor_bytes = Vec::new();
    serde_cbor::to_writer(&mut cbor_bytes, &body)
        .map_err(|e| format!("CBOR encoding failed: {}", e))?;

    let mut hasher = Blake2b::<blake2::digest::consts::U32>::new();
    hasher.update(&cbor_bytes);
    let tx_body_hash = hasher.finalize().to_vec();

    let request = SignWithSchnorrArgument {
        message: tx_body_hash.clone(),
        derivation_path: key_info.derivation_path.clone(),
        key_id: SchnorrKeyId {
            algorithm: SchnorrAlgorithm::Ed25519,
            name: SCHNORR_KEY_NAME.to_string(),
        },
    };


    let (response,): (SignWithSchnorrResponse,) = call_with_payment128(
        Principal::management_canister(),
        "sign_with_schnorr",
        (request,),
        CYCLES_FOR_SIGN_WITH_SCHNORR,
    )
    .await
    .map_err(|e| format!("Signing failed: {:?}", e))?;

    let signature = response.signature;
    if signature.len() != 64 {
        return Err(format!("Invalid Ed25519 signature length: {}", signature.len()));
    }

    let witness_set = Value::Map(vec![
        (Value::Integer(0), Value::Array(vec![Value::Array(vec![
            Value::Bytes(pub_key_bytes),
            Value::Bytes(signature),
        ])])),
    ].into_iter().collect());

    let transaction = Value::Array(vec![
        body,
        witness_set,
        Value::Null,
    ]);

    let mut final_cbor_bytes = Vec::new();
    serde_cbor::to_writer(&mut final_cbor_bytes, &transaction)
        .map_err(|e| format!("CBOR encoding failed: {}", e))?;

    ic_cdk::println!("📦 CBOR transaction hex: {}", hex::encode(&final_cbor_bytes));
    Ok(final_cbor_bytes)
}

async fn submit_transaction(signed_tx: &[u8]) -> Result<String, String> {
    let url = format!("{}/tx/submit", match STATE.with(|s| s.borrow().network.clone()) {
        Network::Mainnet => BLOCKFROST_MAINNET,
        Network::Preprod => BLOCKFROST_PREPROD,
    });
    
    let api_key = STATE.with(|s| s.borrow().blockfrost_api_key.clone());
    
    ic_cdk::println!("Submitting transaction to: {}", url);
    
    let request = CanisterHttpRequestArgument {
        url,
        method: HttpMethod::POST,
        body: Some(signed_tx.to_vec()),
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
        .map_err(|e| format!("HTTP request failed: {:?}", e))?;
    
    ic_cdk::println!("✅ Transaction submission status: {}", response.status);
    
    if response.status != 200u64 {
        let error_body = String::from_utf8_lossy(&response.body);
        ic_cdk::println!("❌ Transaction submission error: {}", error_body);
        return Err(format!("Transaction submission failed: status {}, body: {}", response.status, error_body));
    }
    
    String::from_utf8(response.body)
        .map_err(|e| format!("Invalid response: {}", e))
}


#[update]
pub async fn create_or_sign_cardano_transaction(msg_id: String, to_address: String, amount: String) -> Result<String, String> {
    let amount_ada = amount.parse::<f64>().map_err(|e| format!("Invalid amount: {}", e))?;
    let amount = (amount_ada * LOVELACE_PER_ADA as f64) as u64;
    ic_cdk::println!("Input amount: {} ADA, converted to {} lovelace", amount_ada, amount);

    let state = STATE.with(|s| s.borrow().clone());
    if amount < state.min_utxo {
        return Err(format!(
            "Amount must be at least {} lovelace ({} ADA)",
            state.min_utxo,
            state.min_utxo as f64 / LOVELACE_PER_ADA as f64
        ));
    }

    let caller_principal = caller();
    let (signers, threshold, key_info, network) = STATE.with(|state| {
        let s = state.borrow();
        (
            s.signers.clone(),
            s.threshold,
            s.key_info.clone(),
            s.network.clone(),
        )
    });

    if !signers.contains(&caller_principal) {
        return Err("Caller is not an authorized signer".to_string());
    }

    let expected_prefix = match network {
        Network::Mainnet => "addr",
        Network::Preprod => "addr_test",
    };
    if !to_address.starts_with(expected_prefix) {
        return Err(format!("Invalid to_address: must start with {}", expected_prefix));
    }

    let mut tx = STATE.with(|state| state.borrow().transactions.get(&msg_id).cloned());

    if let Some(ref mut tx) = tx {
        if tx.executed {
            return Err("Transaction already executed".to_string());
        }
        if !tx.signers.contains(&caller_principal) {
            tx.signers.push(caller_principal);
        }
    } else {
        tx = Some(TransactionRecord {
            id: msg_id.clone(),
            to_address,
            amount,
            signers: vec![caller_principal],
            executed: false,
            tx_hash: None,
            timestamp: api::time(),
        });
    }

    let tx = tx.unwrap();
    if tx.signers.len() as u32 >= threshold {
        let key_info = key_info.ok_or("Key info not initialized")?;
        ic_cdk::println!("🔄 Auto-updating UTXOs from blockchain...");
        update_utxos().await?;
        
        let utxos = STATE.with(|s| s.borrow().utxos.clone());
        if utxos.is_empty() {
            return Err("No UTXOs available in your address.".to_string());
        }

        let total_balance: u64 = utxos.iter().map(|u| u.amount).sum();
        ic_cdk::println!(
            "💰 Current balance: {:.6} ADA from {} UTXO(s)",
            total_balance as f64 / LOVELACE_PER_ADA as f64,
            utxos.len()
        );
        ic_cdk::println!("📦 Available UTXOs: {:?}", utxos);


    let (selected, total_input, fee, change) = select_utxos(&utxos, amount)?;

        if total_input < amount + fee {
            return Err(format!(
                "Insufficient funds: need {:.6} ADA (amount) + {:.6} ADA (fee) = {:.6} ADA total, have {:.6} ADA",
                amount as f64 / LOVELACE_PER_ADA as f64,
                fee as f64 / LOVELACE_PER_ADA as f64,
                (amount + fee) as f64 / LOVELACE_PER_ADA as f64,
                total_input as f64 / LOVELACE_PER_ADA as f64
            ));
        }

        ic_cdk::println!(
            "📊 Transaction breakdown:\n   Amount: {} lovelace ({:.6} ADA)\n   Fee: {} lovelace ({:.6} ADA)\n   Change: {} lovelace ({:.6} ADA)\n   Total: {} lovelace ({:.6} ADA)",
            amount,
            amount as f64 / LOVELACE_PER_ADA as f64,
            fee,
            fee as f64 / LOVELACE_PER_ADA as f64,
            change,
            change as f64 / LOVELACE_PER_ADA as f64,
            total_input,
            total_input as f64 / LOVELACE_PER_ADA as f64
        );

        let signed_tx = build_signed_tx(&tx, &selected, change, fee, &key_info).await?;
        let tx_hash = submit_transaction(&signed_tx).await?;

        STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.transactions.insert(
                msg_id.clone(),
                TransactionRecord {
                    id: msg_id.clone(),
                    to_address: tx.to_address,
                    amount: tx.amount,
                    signers: tx.signers,
                    executed: true,
                    tx_hash: Some(tx_hash.clone()),
                    timestamp: tx.timestamp,
                },
            );
        });

        let explorer = get_explorer_url(&tx_hash);
        Ok(format!(
            "Transaction executed!\nTx Hash: {}\nExplorer: {}",
            tx_hash, explorer
        ))
    } else {
        STATE.with(|state| {
            state.borrow_mut().transactions.insert(msg_id.clone(), tx.clone());
        });
        Ok(format!(
            "Signed by {}/{} signers. Waiting for {} more.",
            tx.signers.len(),
            threshold,
            threshold - tx.signers.len() as u32
        ))
    }
}

fn select_utxos(utxos: &[UTXO], amount: u64) -> Result<(Vec<UTXO>, u64, u64, u64), String> {
    let mut selected = Vec::new();
    let mut total = 0u64;
    let state = STATE.with(|s| s.borrow().clone());
    
    // Initial fee estimation (will be recalculated with actual tx size)
    // Assume 1 input + 2 outputs as starting point
    let initial_estimated_size = 250 + 180 + 90; // base + 1 input + 2 outputs
    let base_fee = ((state.min_fee_a * initial_estimated_size + state.min_fee_b) as f64 * 1.1) as u64;
    let min_required = amount + base_fee;

    for utxo in utxos {
        if total < min_required {
            selected.push(utxo.clone());
            total += utxo.amount;
        }
    }

    if total < min_required {
        return Err(format!(
            "Insufficient UTXO balance: need at least {} lovelace ({} ADA)",
            min_required,
            min_required as f64 / LOVELACE_PER_ADA as f64
        ));
    }

    // Calculate fee with improved size estimation
    // Transaction components:
    // - Base transaction structure: ~250 bytes
    // - Each input with witness: ~180 bytes
    // - Each output: ~45 bytes
    // - We have 1-2 outputs (recipient + optional change)
    let num_outputs = if total - amount - (state.min_fee_a * 250 + state.min_fee_b) > 0 { 2u64 } else { 1u64 };
    let estimated_size = 250 + selected.len() as u64 * 180 + num_outputs * 45;
    let mut fee = state.min_fee_a * estimated_size + state.min_fee_b;
    
    // Add 10% safety margin to ensure fee is always sufficient
    fee = (fee as f64 * 1.1) as u64;
    
    let mut change = total - amount - fee;
    
    // Handle case where change is below minimum UTXO value
    if change > 0 && change < state.min_utxo {
        ic_cdk::println!("⚠️  Initial change {} lovelace ({:.6} ADA) is below min_utxo {} lovelace ({:.6} ADA)", 
            change, change as f64 / LOVELACE_PER_ADA as f64,
            state.min_utxo, state.min_utxo as f64 / LOVELACE_PER_ADA as f64);
        
        // Try to add more UTXOs to make change meet minimum
        let remaining_utxos: Vec<&UTXO> = utxos.iter().filter(|u| !selected.contains(u)).collect();
        let mut fixed = false;
        
        for utxo in remaining_utxos {
            selected.push(utxo.clone());
            total += utxo.amount;
            
            // Recalculate fee with improved estimation and safety margin
            let num_outputs_new = if total - amount - (state.min_fee_a * 250 + state.min_fee_b) > 0 { 2u64 } else { 1u64 };
            let estimated_size_new = 250 + selected.len() as u64 * 180 + num_outputs_new * 45;
            let new_fee = ((state.min_fee_a * estimated_size_new + state.min_fee_b) as f64 * 1.1) as u64;
            let new_change = total - amount - new_fee;
            
            if new_change >= state.min_utxo {
                fee = new_fee;
                change = new_change;
                fixed = true;
                ic_cdk::println!("✅ Added UTXO, new change: {} lovelace ({:.6} ADA) - above min_utxo", 
                    change, change as f64 / LOVELACE_PER_ADA as f64);
                break;
            }
        }
        
        // If we couldn't fix it by adding UTXOs, error out with helpful message
        if !fixed {
            // Calculate the options
            let add_amount = state.min_utxo - change;
            let new_amount_option1 = amount + add_amount;
            let new_change_option1 = state.min_utxo;
            
            // Option 2: Send maximum possible (total - fee) to eliminate change
            let max_sendable = total.saturating_sub(fee);
            
            // Option 3: Send less to make change >= min_utxo
            let max_amount_for_valid_change = total.saturating_sub(state.min_utxo).saturating_sub(fee);
            let reduce_amount = amount.saturating_sub(max_amount_for_valid_change);
            
            return Err(format!(
                "❌ Transaction would create a change output of {} lovelace ({:.6} ADA), which is below the minimum UTXO requirement of {} lovelace ({:.6} ADA).\n\n\
                 💡 To fix this, you can either:\n\
                 1️⃣  SEND MORE: Add {} lovelace to your amount\n\
                    → Send {:.6} ADA total (change will be {:.6} ADA ✓)\n\n\
                 2️⃣  SEND ALMOST EVERYTHING: Send the maximum possible\n\
                    → Send {:.6} ADA total (no change output, all consumed)\n\n\
                 3️⃣  SEND LESS: Reduce amount by {} lovelace\n\
                    → Send {:.6} ADA total (change will be ~{:.6} ADA ✓)",
                change,
                change as f64 / LOVELACE_PER_ADA as f64,
                state.min_utxo,
                state.min_utxo as f64 / LOVELACE_PER_ADA as f64,
                add_amount,
                new_amount_option1 as f64 / LOVELACE_PER_ADA as f64,
                new_change_option1 as f64 / LOVELACE_PER_ADA as f64,
                max_sendable as f64 / LOVELACE_PER_ADA as f64,
                reduce_amount,
                max_amount_for_valid_change as f64 / LOVELACE_PER_ADA as f64,
                state.min_utxo as f64 / LOVELACE_PER_ADA as f64
            ));
        }
    }

    ic_cdk::println!("✅ UTXO selection complete:");
    ic_cdk::println!("   Selected {} UTXOs", selected.len());
    ic_cdk::println!("   Total input: {} lovelace ({:.6} ADA)", total, total as f64 / LOVELACE_PER_ADA as f64);
    ic_cdk::println!("   Amount to send: {} lovelace ({:.6} ADA)", amount, amount as f64 / LOVELACE_PER_ADA as f64);
    ic_cdk::println!("   Fee: {} lovelace ({:.6} ADA)", fee, fee as f64 / LOVELACE_PER_ADA as f64);
    ic_cdk::println!("   Change: {} lovelace ({:.6} ADA)", change, change as f64 / LOVELACE_PER_ADA as f64);
    
    Ok((selected, total, fee, change))
}

fn decode_address(address: &str) -> Result<Vec<u8>, String> {
    let (_hrp, data, _variant) = bech32::decode(address).map_err(|e| format!("Failed to decode address: {}", e))?;
    let bytes = bech32::convert_bits(&data, 5, 8, false).map_err(|e| format!("Failed to convert bits: {}", e))?;
    Ok(bytes)
}

fn get_explorer_url(tx_hash: &str) -> String {
    format!("https://cardanoscan.io/transaction/{}", tx_hash)
}

#[pre_upgrade]
fn pre_upgrade() {
    let state = STATE.with(|s| s.borrow().clone());
    ic_cdk::storage::stable_save((state,)).expect("Failed to save state");
}

#[post_upgrade]
fn post_upgrade() {
    let (state,): (StableState,) = ic_cdk::storage::stable_restore()
        .expect("Failed to restore state");
    STATE.with(|s| *s.borrow_mut() = state);
}