use candid::{CandidType, Deserialize};
use ic_cdk::api::management_canister::ecdsa::{
    sign_with_ecdsa, ecdsa_public_key, SignWithEcdsaArgument, SignWithEcdsaResponse,
    EcdsaPublicKeyArgument, EcdsaPublicKeyResponse, EcdsaKeyId, EcdsaCurve,
};
use ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpMethod, HttpHeader, HttpResponse, TransformArgs,
    TransformContext,
};
use ic_cdk::{query, update, caller};
use serde::Serialize;
use serde_json::{json, Value, to_vec};
use std::collections::HashMap;
use hex;
use sha3::{Keccak256, Digest};
use k256::{
    ecdsa::{RecoveryId, Signature, VerifyingKey},
    elliptic_curve::sec1::ToEncodedPoint,
    PublicKey,
};
use k256::elliptic_curve::generic_array::GenericArray;
use std::convert::TryFrom;

// Ethereum transaction structures
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct EthTransaction {
    pub nonce: u64,
    pub gas_price: String,        // In wei
    pub gas_limit: u64,
    pub to: String,               // Ethereum address (0x...)
    pub value: String,            // Amount in wei
    pub data: Vec<u8>,            // Transaction data (empty for simple transfers)
    pub chain_id: u64,            // 1 for mainnet, 11155111 for sepolia testnet
    pub from: String,             // Sender address
    pub signature: Option<EthSignature>,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct EthSignature {
    pub r: Vec<u8>,
    pub s: Vec<u8>,
    pub v: u64,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct EthTransactionRecord {
    pub id: String,
    pub transaction: EthTransaction,
    pub signers: Vec<candid::Principal>,
    pub executed: bool,
    pub tx_hash: Option<String>,
    pub timestamp: u64,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct EthKeyInfo {
    pub public_key: Vec<u8>,        // secp256k1 public key (65 bytes uncompressed)
    pub compressed_key: Vec<u8>,    // secp256k1 compressed public key (33 bytes)
    pub eth_address: String,        // Derived Ethereum address (0x...)
    pub derivation_path: Vec<Vec<u8>>, // Key derivation path
}

// Global state for Ethereum transactions
thread_local! {
    static ETH_STATE: std::cell::RefCell<EthState> = std::cell::RefCell::new(EthState::default());
    static ECDSA_KEY_NAME: std::cell::RefCell<String> = std::cell::RefCell::new("key_1".to_string());
}

#[derive(Default)]
struct EthState {
    transactions: HashMap<String, EthTransactionRecord>,
    signers: Vec<candid::Principal>,
    threshold: u32,
    eth_key_info: Option<EthKeyInfo>,
    nonce: u64, // Track account nonce
}

// PRODUCTION: Proper Keccak256 implementation
fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let result = hasher.finalize();
    result.into()
}

// PRODUCTION: Proper secp256k1 point compression
fn compress_secp256k1_public_key(uncompressed_key: &[u8]) -> Result<Vec<u8>, String> {
    if uncompressed_key.len() != 65 {
        return Err("Invalid uncompressed secp256k1 public key length, expected 65 bytes".to_string());
    }

    if uncompressed_key[0] != 0x04 {
        return Err("Invalid uncompressed public key format, must start with 0x04".to_string());
    }

    // Use proper elliptic curve library for compression
    let public_key = PublicKey::from_sec1_bytes(uncompressed_key)
        .map_err(|e| format!("Failed to parse public key: {}", e))?;
    
    let compressed_point = public_key.to_encoded_point(true);
    Ok(compressed_point.as_bytes().to_vec())
}

// PRODUCTION: Proper secp256k1 point decompression
fn decompress_secp256k1_public_key(compressed_key: &[u8]) -> Result<Vec<u8>, String> {
    if compressed_key.len() != 33 {
        return Err("Invalid compressed secp256k1 public key length, expected 33 bytes".to_string());
    }

    // Use proper elliptic curve library for decompression
    let public_key = PublicKey::from_sec1_bytes(compressed_key)
        .map_err(|e| format!("Failed to parse compressed key: {}", e))?;
    
    let uncompressed_point = public_key.to_encoded_point(false);
    Ok(uncompressed_point.as_bytes().to_vec())
}

// PRODUCTION: Proper Ethereum address derivation
fn derive_eth_address_from_secp256k1(public_key: &[u8]) -> Result<String, String> {
    // Handle both compressed (33 bytes) and uncompressed (65 bytes) keys
    let uncompressed_key = match public_key.len() {
        65 => {
            // Already uncompressed
            if public_key[0] != 0x04 {
                return Err("Invalid uncompressed public key format".to_string());
            }
            public_key.to_vec()
        },
        33 => {
            // Decompress the key
            decompress_secp256k1_public_key(public_key)?
        },
        _ => {
            return Err(format!("Invalid secp256k1 public key length: {} bytes", public_key.len()));
        }
    };

    // Ethereum address derivation:
    // 1. Take the uncompressed public key (without the 0x04 prefix)
    // 2. Keccak256 hash of the 64-byte public key
    // 3. Take the last 20 bytes
    // 4. Prefix with 0x

    // Remove the 0x04 prefix to get the 64-byte key
    let key_without_prefix = &uncompressed_key[1..];

    // Proper Keccak256 hash
    let hash_result = keccak256(key_without_prefix);

    // Take the last 20 bytes
    let address_bytes = &hash_result[12..32];

    // Format as 0x + hex
    let address = format!("0x{}", hex::encode(address_bytes));
    Ok(address)
}

// Helper function to get appropriate ECDSA key name based on environment
fn get_ecdsa_key_name() -> String {
    // ECDSA_KEY_NAME.with(|key| key.borrow().clone())
    "key_1".to_string()
}

// Configure ECDSA key name for different environments
#[update]
pub fn set_ecdsa_key_name(key_name: String) -> Result<String, String> {
    match key_name.as_str() {
        "key_1" | "dfx_test_key" => {
            ECDSA_KEY_NAME.with(|key| {
                *key.borrow_mut() = key_name.clone();
            });
            
            // Clear cached key info when switching keys
            ETH_STATE.with(|state| {
                state.borrow_mut().eth_key_info = None;
            });
            
            Ok(format!("ECDSA key name set to: {}", key_name))
        },
        _ => Err("Invalid key name. Use 'key_1' (production/testnet) or 'dfx_test_key' (local testing)".to_string())
    }
}

// Get Ethereum public key and address using secp256k1 ECDSA
#[update]
pub async fn get_eth_key_info() -> Result<EthKeyInfo, String> {
    let derivation_path = vec![b"eth".to_vec(), caller().as_slice().to_vec()];
    
    // Get secp256k1 public key from IC
    let request = EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path: derivation_path.clone(),
        key_id: EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: get_ecdsa_key_name(),
        },
    };

    let (response,): (EcdsaPublicKeyResponse,) = ecdsa_public_key(request)
        .await
        .map_err(|e| format!("Failed to get public key: {:?}", e))?;

    let public_key = response.public_key;
    
    // Handle both compressed and uncompressed keys
    let (uncompressed_key, compressed_key) = match public_key.len() {
        33 => {
            // Key is compressed, decompress it
            let uncompressed = decompress_secp256k1_public_key(&public_key)?;
            (uncompressed, public_key.clone())
        },
        65 => {
            // Key is uncompressed, compress it for storage
            let compressed = compress_secp256k1_public_key(&public_key)?;
            (public_key.clone(), compressed)
        },
        _ => {
            return Err(format!("Unexpected public key length: {} bytes", public_key.len()));
        }
    };
    
    // Derive Ethereum address using the uncompressed key
    let eth_address = derive_eth_address_from_secp256k1(&uncompressed_key)?;

    let key_info = EthKeyInfo {
        public_key: uncompressed_key,
        compressed_key,
        eth_address: eth_address.clone(),
        derivation_path,
    };

    // Store in state
    ETH_STATE.with(|state| {
        state.borrow_mut().eth_key_info = Some(key_info.clone());
    });

    ic_cdk::println!("Generated Ethereum address: {} using ECDSA key: {}", 
        eth_address, get_ecdsa_key_name());

    Ok(key_info)
}

// Estimate gas for Ethereum transaction
pub async fn estimate_eth_gas(
    from_address: &str,
    to_address: &str,
    value: &str,
    data: &[u8],
) -> Result<u64, String> {
    let rpc_endpoints = vec![
        "https://1rpc.io/eth",
        "https://ethereum.publicnode.com",
        "https://rpc.ankr.com/eth",
    ];
    
    let mut last_error = String::new();
    
    for rpc_url in rpc_endpoints {
        match estimate_gas_single_endpoint(rpc_url, from_address, to_address, value, data).await {
            Ok(gas_estimate) => {
                ic_cdk::println!("Gas estimation successful: {} gas units", gas_estimate);
                // Add 20% buffer to the estimate
                let gas_with_buffer = (gas_estimate as f64 * 1.2) as u64;
                return Ok(gas_with_buffer);
            },
            Err(e) => {
                last_error = format!("Failed with {}: {}", rpc_url, e);
                ic_cdk::println!("Gas estimation failed for {}: {}", rpc_url, e);
                continue;
            }
        }
    }
    
    // Fallback to reasonable defaults if all estimations fail
    let fallback_gas = if data.is_empty() { 21000 } else { 100000 };
    ic_cdk::println!("All gas estimation endpoints failed, using fallback: {} gas units", fallback_gas);
    Ok(fallback_gas)
}

async fn estimate_gas_single_endpoint(
    rpc_url: &str,
    from_address: &str,
    to_address: &str,
    value: &str,
    data: &[u8],
) -> Result<u64, String> {
    let mut params = json!({
        "from": from_address,
        "value": format!("0x{:x}", value.parse::<u128>().unwrap_or(0))
    });
    
    if !to_address.is_empty() {
        params["to"] = json!(to_address);
    }
    
    if !data.is_empty() {
        params["data"] = json!(format!("0x{}", hex::encode(data)));
    }

    let gas_request = json!({
        "method": "eth_estimateGas",
        "params": [params],
        "jsonrpc": "2.0",
        "id": 1
    });

    let request_body = gas_request.to_string();
    let request_headers = vec![
        HttpHeader {
            name: "Content-Type".to_string(),
            value: "application/json".to_string(),
        },
    ];

    let request = CanisterHttpRequestArgument {
        url: rpc_url.to_string(),
        method: HttpMethod::POST,
        body: Some(request_body.as_bytes().to_vec()),
        max_response_bytes: Some(4096),
        transform: Some(TransformContext::from_name("transform".to_string(), to_vec(&()).unwrap())),
        headers: request_headers,
    };

    let (response,): (HttpResponse,) = http_request(request, 3_000_000_000)
        .await
        .map_err(|e| format!("HTTP request failed: {:?}", e))?;

    let response_body = String::from_utf8(response.body)
        .map_err(|e| format!("Failed to parse response body: {}", e))?;

    let response_json: Value = serde_json::from_str(&response_body)
        .map_err(|e| format!("Failed to parse JSON response: {}. Raw: {}", e, response_body))?;

    if let Some(error) = response_json.get("error") {
        return Err(format!("RPC Error: {}", error));
    }

    if let Some(result) = response_json.get("result") {
        if let Some(gas_hex) = result.as_str() {
            let gas_estimate = u64::from_str_radix(gas_hex.strip_prefix("0x").unwrap_or(gas_hex), 16)
                .map_err(|_| "Failed to parse gas estimate")?;
            return Ok(gas_estimate);
        }
    }

    Err("Could not retrieve gas estimate".to_string())
}

// PRODUCTION: RLP encoding functions with proper error handling
fn rlp_encode_u64(value: u64) -> Vec<u8> {
    if value == 0 {
        return vec![0x80]; // Empty byte string
    }
    let bytes = value.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(7);
    rlp_encode_bytes(&bytes[start..])
}

fn rlp_encode_u128(value: u128) -> Vec<u8> {
    if value == 0 {
        return vec![0x80]; // Empty byte string
    }
    let bytes = value.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(15);
    rlp_encode_bytes(&bytes[start..])
}

fn rlp_encode_bytes(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return vec![0x80];
    }
    
    if data.len() == 1 && data[0] < 0x80 {
        return data.to_vec();
    }
    
    if data.len() < 56 {
        let mut result = vec![0x80 + data.len() as u8];
        result.extend_from_slice(data);
        result
    } else {
        let len_bytes = (data.len() as u64).to_be_bytes();
        let start = len_bytes.iter().position(|&b| b != 0).unwrap_or(7);
        let len_encoding = &len_bytes[start..];
        
        let mut result = vec![0xb7 + len_encoding.len() as u8];
        result.extend_from_slice(len_encoding);
        result.extend_from_slice(data);
        result
    }
}

fn rlp_encode_list(items: &[Vec<u8>]) -> Vec<u8> {
    let mut content = Vec::new();
    for item in items {
        content.extend_from_slice(item);
    }
    
    if content.len() < 56 {
        let mut result = vec![0xc0 + content.len() as u8];
        result.extend_from_slice(&content);
        result
    } else {
        let len_bytes = (content.len() as u64).to_be_bytes();
        let start = len_bytes.iter().position(|&b| b != 0).unwrap_or(7);
        let len_encoding = &len_bytes[start..];
        
        let mut result = vec![0xf7 + len_encoding.len() as u8];
        result.extend_from_slice(len_encoding);
        result.extend_from_slice(&content);
        result
    }
}

// PRODUCTION: EIP-155 compliant RLP encoding
fn rlp_encode_transaction(tx: &EthTransaction) -> Result<Vec<u8>, String> {
    let mut rlp_items = Vec::new();
    
    // EIP-155 order: [nonce, gasPrice, gasLimit, to, value, data, v, r, s] for signed
    // or [nonce, gasPrice, gasLimit, to, value, data, chainId, 0, 0] for signing
    
    rlp_items.push(rlp_encode_u64(tx.nonce));
    
    let gas_price_wei: u128 = tx.gas_price.parse()
        .map_err(|_| "Invalid gas price format")?;
    rlp_items.push(rlp_encode_u128(gas_price_wei));
    
    rlp_items.push(rlp_encode_u64(tx.gas_limit));
    
    if tx.to.is_empty() {
        rlp_items.push(rlp_encode_bytes(&[])); // Empty for contract creation
    } else {
        let to_bytes = hex::decode(tx.to.strip_prefix("0x").unwrap_or(&tx.to))
            .map_err(|_| "Invalid 'to' address hex")?;
        if to_bytes.len() != 20 {
            return Err("Invalid 'to' address length, expected 20 bytes".to_string());
        }
        rlp_items.push(rlp_encode_bytes(&to_bytes));
    }
    
    let value_wei: u128 = tx.value.parse()
        .map_err(|_| "Invalid value format")?;
    rlp_items.push(rlp_encode_u128(value_wei));
    
    rlp_items.push(rlp_encode_bytes(&tx.data));
    
    if let Some(signature) = &tx.signature {
        // For signed transactions: v, r, s
        rlp_items.push(rlp_encode_u64(signature.v));
        rlp_items.push(rlp_encode_bytes(&signature.r));
        rlp_items.push(rlp_encode_bytes(&signature.s));
    } else {
        // For signing: chainId, 0, 0 (EIP-155)
        rlp_items.push(rlp_encode_u64(tx.chain_id));
        rlp_items.push(rlp_encode_bytes(&[])); // r = 0
        rlp_items.push(rlp_encode_bytes(&[])); // s = 0
    }
    
    Ok(rlp_encode_list(&rlp_items))
}

// PRODUCTION: Create Ethereum transaction hash for signing
fn create_eth_transaction_hash(tx: &EthTransaction) -> Result<Vec<u8>, String> {
    let unsigned_tx = EthTransaction {
        signature: None,
        ..tx.clone()
    };
    
    let rlp_encoded = rlp_encode_transaction(&unsigned_tx)?;
    
    // Proper Keccak256 hash for Ethereum
    let hash = keccak256(&rlp_encoded);
    
    Ok(hash.to_vec())
}

// Helper function to strip leading zeros from byte array (for canonical RLP)
fn strip_leading_zeros(bytes: &[u8]) -> Vec<u8> {
    let start_pos = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len() - 1);
    
    // Always keep at least one byte (even if it's zero)
    if start_pos == bytes.len() {
        vec![0u8]
    } else {
        bytes[start_pos..].to_vec()
    }
}

// PRODUCTION: Convert ECDSA signature to Ethereum format with proper recovery


fn convert_ecdsa_to_eth_signature(
    ecdsa_signature: &[u8],
    message_hash: &[u8],
    expected_address: &str,
    chain_id: u64,
) -> Result<EthSignature, String> {
    if ecdsa_signature.len() != 64 {
        return Err("Invalid ECDSA signature length, expected 64 bytes".to_string());
    }

    // Create signature directly from the 64-byte array
    let signature = Signature::try_from(ecdsa_signature)
        .map_err(|e| format!("Failed to create signature from bytes: {}", e))?;

    let r_bytes = &ecdsa_signature[0..32];
    let s_bytes = &ecdsa_signature[32..64];

    // Try both recovery IDs
    for recovery_id in [0u8, 1u8] {
        if let Some(rec_id) = RecoveryId::from_byte(recovery_id) {
            if let Ok(recovered_key) = VerifyingKey::recover_from_prehash(
                message_hash, 
                &signature, 
                rec_id
            ) {
                let public_key_point = recovered_key.to_encoded_point(false);
                let public_key_bytes = public_key_point.as_bytes();
                
                if let Ok(recovered_address) = derive_eth_address_from_secp256k1(public_key_bytes) {
                    if recovered_address.to_lowercase() == expected_address.to_lowercase() {
                        let v = (recovery_id as u64) + 35 + 2 * chain_id; // EIP-155
                        
                        ic_cdk::println!(
                            "Signature recovery successful: recovery_id={}, v={}, address={}",
                            recovery_id, v, recovered_address
                        );
                        
                        return Ok(EthSignature {
                            r: strip_leading_zeros(r_bytes),
                            s: strip_leading_zeros(s_bytes),
                            v,
                        });
                    }
                }
            }
        }
    }

    Err(format!(
        "Failed to recover correct address. Expected: {}, but signature does not match.",
        expected_address
    ))
}

// Dynamic gas price with EIP-1559 support
async fn query_eth_gas_price() -> Result<String, String> {
    let rpc_endpoints = vec![
        "https://1rpc.io/eth",
        "https://ethereum.publicnode.com", 
        "https://rpc.ankr.com/eth",
    ];
    
    for rpc_url in rpc_endpoints {
        match get_gas_price_from_endpoint(rpc_url).await {
            Ok(gas_price) => {
                let gas_price_wei = gas_price.parse::<u128>().unwrap_or(2_000_000_000);
                let minimum_gas_with_tip = gas_price_wei.max(2_000_000_000); // At least 2 Gwei total
                
                ic_cdk::println!("Retrieved gas price: {} wei ({:.2} Gwei)", 
                    minimum_gas_with_tip, 
                    (minimum_gas_with_tip as f64) / 1e9
                );
                return Ok(minimum_gas_with_tip.to_string());
            },
            Err(e) => {
                ic_cdk::println!("Failed to get gas price from {}: {}", rpc_url, e);
                continue;
            }
        }
    }
    
    // Fallback: 3 Gwei
    let fallback_gas_price = "3000000000".to_string();
    ic_cdk::println!("Using fallback gas price: 3 Gwei");
    Ok(fallback_gas_price)
}

async fn get_gas_price_from_endpoint(rpc_url: &str) -> Result<String, String> {
    // Try EIP-1559 first, then legacy
    match get_eip1559_gas_price(rpc_url).await {
        Ok(price) => Ok(price),
        Err(_) => get_legacy_gas_price(rpc_url).await,
    }
}

async fn get_eip1559_gas_price(rpc_url: &str) -> Result<String, String> {
    let gas_request = json!({
        "method": "eth_feeHistory", 
        "params": [1, "latest", [50]],
        "jsonrpc": "2.0",
        "id": 1
    });

    let request_body = gas_request.to_string();
    let request = CanisterHttpRequestArgument {
        url: rpc_url.to_string(),
        method: HttpMethod::POST,
        body: Some(request_body.as_bytes().to_vec()),
        max_response_bytes: Some(4096),
        transform: Some(TransformContext::from_name("transform".to_string(), to_vec(&()).unwrap())),
        headers: vec![
            HttpHeader {
                name: "Content-Type".to_string(),
                value: "application/json".to_string(),
            },
        ],
    };

    let (response,): (HttpResponse,) = http_request(request, 3_000_000_000).await
        .map_err(|e| format!("HTTP request failed: {:?}", e))?;

    let response_body = String::from_utf8(response.body)
        .map_err(|e| format!("Failed to parse response body: {}", e))?;

    let response_json: Value = serde_json::from_str(&response_body)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    if let Some(result) = response_json.get("result") {
        if let (Some(base_fee_array), Some(priority_fee_array)) = (
            result.get("baseFeePerGas").and_then(|v| v.as_array()),
            result.get("reward").and_then(|v| v.as_array())
        ) {
            if let (Some(base_fee_hex), Some(priority_fees)) = (
                base_fee_array.get(1).and_then(|v| v.as_str()),
                priority_fee_array.get(0).and_then(|v| v.as_array())
            ) {
                let base_fee = u128::from_str_radix(
                    base_fee_hex.strip_prefix("0x").unwrap_or(base_fee_hex), 16
                ).map_err(|_| "Invalid base fee")?;

                let priority_fee = if let Some(priority_hex) = priority_fees.get(0).and_then(|v| v.as_str()) {
                    let network_priority = u128::from_str_radix(
                        priority_hex.strip_prefix("0x").unwrap_or(priority_hex), 16
                    ).unwrap_or(1_500_000_000);
                    network_priority.max(1_500_000_000) // Min 1.5 Gwei priority
                } else {
                    1_500_000_000
                };

                let total_gas_price = base_fee + priority_fee;
                let minimum_total_gas = total_gas_price.max(2_500_000_000); // Min 2.5 Gwei total
                
                return Ok(minimum_total_gas.to_string());
            }
        }
    }

    Err("Could not parse EIP-1559 fee data".to_string())
}

async fn get_legacy_gas_price(rpc_url: &str) -> Result<String, String> {
    let gas_request = json!({
        "method": "eth_gasPrice",
        "params": [],
        "jsonrpc": "2.0", 
        "id": 1
    });

    let request = CanisterHttpRequestArgument {
        url: rpc_url.to_string(),
        method: HttpMethod::POST,
        body: Some(gas_request.to_string().as_bytes().to_vec()),
        max_response_bytes: Some(4096),
        transform: Some(TransformContext::from_name("transform".to_string(), to_vec(&()).unwrap())),
        headers: vec![
            HttpHeader {
                name: "Content-Type".to_string(),
                value: "application/json".to_string(),
            },
        ],
    };

    let (response,): (HttpResponse,) = http_request(request, 3_000_000_000).await
        .map_err(|e| format!("HTTP request failed: {:?}", e))?;

    let response_body = String::from_utf8(response.body)
        .map_err(|e| format!("Failed to parse response body: {}", e))?;

    let response_json: Value = serde_json::from_str(&response_body)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    if let Some(result) = response_json.get("result") {
        if let Some(gas_hex) = result.as_str() {
            let network_gas_price = u128::from_str_radix(
                gas_hex.strip_prefix("0x").unwrap_or(gas_hex), 16
            ).map_err(|_| "Failed to parse gas price")?;
            
            let minimum_gas_price = network_gas_price.max(2_500_000_000); // Min 2.5 Gwei
            return Ok(minimum_gas_price.to_string());
        }
    }

    Err("Could not retrieve gas price".to_string())
}


pub async fn create_or_sign_eth_transaction_with_gas_estimation(
    msg_id: String,
    to_address: String,
    amount_eth: String,
    gas_price_gwei: u64,
) -> Result<String, String> {
    let caller = caller();

    // Input validation
    if msg_id.trim().is_empty() {
        return Err("Transaction ID cannot be empty".to_string());
    }

    if to_address.trim().is_empty() {
        return Err("Destination address cannot be empty".to_string());
    }

    if !to_address.starts_with("0x") || to_address.len() != 42 {
        return Err("Invalid Ethereum address format".to_string());
    }

    // Validate hex characters in address
    if hex::decode(to_address.strip_prefix("0x").unwrap_or(&to_address)).is_err() {
        return Err("Invalid hex characters in address".to_string());
    }

    let (is_authorized, key_info, threshold, current_nonce) = ETH_STATE.with(|state| {
        let s = state.borrow();
        let is_authorized = s.signers.contains(&caller);
        let key_info = s.eth_key_info.clone();
        (is_authorized, key_info, s.threshold, s.nonce)
    });

    if !is_authorized {
        return Err("Caller is not an authorized signer".to_string());
    }

    let key_info = key_info.ok_or("Ethereum key info not initialized. Call get_eth_key_info first")?;

    // Convert ETH to wei with precision validation
    let amount_f64: f64 = amount_eth.parse().map_err(|_| "Invalid amount format")?;
    if amount_f64 <= 0.0 {
        return Err("Amount must be greater than 0".to_string());
    }
    
    // Check for reasonable bounds (prevent overflow)
    if amount_f64 > 1e10 {
        return Err("Amount too large".to_string());
    }
    
    let amount_wei = ((amount_f64 * 1e18) as u128).to_string();

    // Estimate gas for the transaction
    let estimated_gas = estimate_eth_gas(&key_info.eth_address, &to_address, &amount_wei, &[]).await?;
    
    // Validate gas price (minimum 1 Gwei for network acceptance)
    let minimum_gas_price_gwei = gas_price_gwei.max(1);
    let gas_price = ((minimum_gas_price_gwei as u128) * 1_000_000_000).to_string();
    
    let network_nonce = query_eth_nonce(&key_info.eth_address).await.unwrap_or(current_nonce);

    let (transaction_exists, _) = ETH_STATE.with(|state| {
        let s = state.borrow();
        let exists = s.transactions.contains_key(&msg_id);
        (exists, exists)
    });

    if threshold == 1 {
        // Single-signer: execute immediately
        if transaction_exists {
            return Err("Transaction already exists".to_string());
        }

        let transaction = EthTransaction {
            nonce: network_nonce,
            gas_price,
            gas_limit: estimated_gas,
            to: to_address,
            value: amount_wei,
            data: vec![],
            chain_id: 1, // Mainnet
            from: key_info.eth_address,
            signature: None,
        };

        let tx_record = EthTransactionRecord {
            id: msg_id.clone(),
            transaction,
            signers: vec![caller],
            executed: false,
            tx_hash: None,
            timestamp: ic_cdk::api::time(),
        };

        ETH_STATE.with(|state| {
            state.borrow_mut().transactions.insert(msg_id.clone(), tx_record);
        });

        execute_eth_transaction(msg_id.clone()).await
    } else {
        // Multi-signer logic
        if transaction_exists {
            let should_execute = ETH_STATE.with(|state| {
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
                execute_eth_transaction(msg_id.clone()).await
            } else {
                let signer_count = ETH_STATE.with(|s| 
                    s.borrow().transactions.get(&msg_id).unwrap().signers.len()
                );
                Ok(format!("Transaction {} signed. {}/{} signatures", 
                    msg_id, signer_count, threshold))
            }
        } else {
            let transaction = EthTransaction {
                nonce: network_nonce,
                gas_price,
                gas_limit: estimated_gas,
                to: to_address,
                value: amount_wei,
                data: vec![],
                chain_id: 1, // Mainnet
                from: key_info.eth_address.clone(),
                signature: None,
            };

            let tx_record = EthTransactionRecord {
                id: msg_id.clone(),
                transaction,
                signers: vec![caller],
                executed: false,
                tx_hash: None,
                timestamp: ic_cdk::api::time(),
            };

            ETH_STATE.with(|state| {
                state.borrow_mut().transactions.insert(msg_id.clone(), tx_record);
            });

            Ok(format!("Ethereum transaction {} created with {} Gwei gas price and {} gas limit. 1/{} signatures", 
                msg_id, minimum_gas_price_gwei, estimated_gas, threshold))
        }
    }
}

// PRODUCTION: Execute Ethereum transaction with comprehensive error handling
async fn execute_eth_transaction(transaction_id: String) -> Result<String, String> {
    let (mut transaction, key_info) = ETH_STATE.with(|state| {
        let s = state.borrow();
        if let Some(tx_record) = s.transactions.get(&transaction_id) {
            let key_info = s.eth_key_info.clone().ok_or("Ethereum key info not available")?;
            Ok((tx_record.transaction.clone(), key_info))
        } else {
            Err("Transaction not found".to_string())
        }
    })?;

    ic_cdk::println!("Executing ETH transaction: nonce={}, gas_price={}, gas_limit={}, value={}", 
        transaction.nonce, transaction.gas_price, transaction.gas_limit, transaction.value);

    // Create transaction hash for signing
    let tx_hash = create_eth_transaction_hash(&transaction)?;
    ic_cdk::println!("Transaction hash for signing: {}", hex::encode(&tx_hash));

    // Sign with IC ECDSA
    let request = SignWithEcdsaArgument {
        message_hash: tx_hash.clone(),
        derivation_path: key_info.derivation_path.clone(),
        key_id: EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: get_ecdsa_key_name(),
        },
    };

    let (response,): (SignWithEcdsaResponse,) = sign_with_ecdsa(request)
        .await
        .map_err(|e| format!("Failed to sign transaction: {:?}", e))?;

    ic_cdk::println!("ECDSA signature received: {} bytes", response.signature.len());

    // Convert to Ethereum signature with proper recovery
    let eth_signature = convert_ecdsa_to_eth_signature(
        &response.signature, 
        &tx_hash,
        &transaction.from, 
        transaction.chain_id
    )?;
    
    transaction.signature = Some(eth_signature);

    // Create RLP-encoded transaction
    let rlp_encoded = rlp_encode_transaction(&transaction)
        .map_err(|e| format!("RLP encoding failed: {}", e))?;
    let tx_hex = format!("0x{}", hex::encode(&rlp_encoded));

    ic_cdk::println!("RLP encoded transaction ({} bytes): {}", rlp_encoded.len(), tx_hex);

    // Submit to network
    let submission_result = submit_eth_transaction_to_network(&tx_hex).await?;
    
    // Update state
    ETH_STATE.with(|state| {
        let mut s = state.borrow_mut();
        if let Some(tx_record) = s.transactions.get_mut(&transaction_id) {
            tx_record.executed = true;
            tx_record.tx_hash = Some(submission_result.clone());
            tx_record.transaction.signature = transaction.signature.clone();
            s.nonce = transaction.nonce + 1;
        }
    });

    Ok(format!("Ethereum transaction {} executed successfully. Hash: {}", transaction_id, submission_result))
}

// Submit transaction with retry logic
async fn submit_eth_transaction_to_network(tx_hex: &str) -> Result<String, String> {
    let rpc_endpoints = vec![
        "https://1rpc.io/eth",
        "https://ethereum.publicnode.com",
        "https://rpc.ankr.com/eth",
    ];
    
    let mut last_error = String::new();
    
    for rpc_url in rpc_endpoints {
        match submit_to_single_eth_endpoint(rpc_url, tx_hex).await {
            Ok(result) => {
                ic_cdk::println!("Transaction submitted successfully via {}", rpc_url);
                return Ok(result);
            },
            Err(e) => {
                last_error = format!("Failed with {}: {}", rpc_url, e);
                ic_cdk::println!("RPC endpoint failed: {}", last_error);
                continue;
            }
        }
    }
    
    Err(format!("All Ethereum RPC endpoints failed. Last error: {}", last_error))
}

async fn submit_to_single_eth_endpoint(rpc_url: &str, tx_hex: &str) -> Result<String, String> {
    let submit_request = json!({
        "method": "eth_sendRawTransaction",
        "params": [tx_hex],
        "jsonrpc": "2.0",
        "id": 1
    });

    let request = CanisterHttpRequestArgument {
        url: rpc_url.to_string(),
        method: HttpMethod::POST,
        body: Some(submit_request.to_string().as_bytes().to_vec()),
        max_response_bytes: Some(8192),
        transform: Some(TransformContext::from_name("transform".to_string(), to_vec(&()).unwrap())),
        headers: vec![
            HttpHeader {
                name: "Content-Type".to_string(),
                value: "application/json".to_string(),
            },
        ],
    };

    let (response,): (HttpResponse,) = http_request(request, 5_000_000_000)
        .await
        .map_err(|e| format!("HTTP request failed: {:?}", e))?;

    let response_body = String::from_utf8(response.body)
        .map_err(|e| format!("Failed to parse response body: {}", e))?;

    let response_json: Value = serde_json::from_str(&response_body)
        .map_err(|e| format!("Failed to parse JSON response: {}. Raw response: {}", e, response_body))?;

    if let Some(error) = response_json.get("error") {
        return Err(format!("RPC Error: {}", error));
    }

    if let Some(result) = response_json.get("result") {
        if let Some(tx_hash) = result.as_str() {
            return Ok(tx_hash.to_string());
        }
    }

    Err(format!("Unexpected response format. Full response: {}", response_body))
}

// Utility functions for querying network state
pub async fn query_eth_nonce(address: &str) -> Result<u64, String> {
    let rpc_url = "https://1rpc.io/eth";
    
    let nonce_request = json!({
        "method": "eth_getTransactionCount",
        "params": [address, "latest"],
        "jsonrpc": "2.0",
        "id": 1
    });

    let request = CanisterHttpRequestArgument {
        url: rpc_url.to_string(),
        method: HttpMethod::POST,
        body: Some(nonce_request.to_string().as_bytes().to_vec()),
        max_response_bytes: Some(4096),
        transform: Some(TransformContext::from_name("transform".to_string(), to_vec(&()).unwrap())),
        headers: vec![
            HttpHeader {
                name: "Content-Type".to_string(),
                value: "application/json".to_string(),
            },
        ],
    };

    let (response,): (HttpResponse,) = http_request(request, 3_000_000_000)
        .await
        .map_err(|e| format!("HTTP request failed: {:?}", e))?;

    let response_body = String::from_utf8(response.body)
        .map_err(|e| format!("Failed to parse response body: {}", e))?;

    let response_json: Value = serde_json::from_str(&response_body)
        .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

    if let Some(result) = response_json.get("result") {
        if let Some(nonce_hex) = result.as_str() {
            let nonce = u64::from_str_radix(nonce_hex.strip_prefix("0x").unwrap_or(nonce_hex), 16)
                .map_err(|_| "Failed to parse nonce")?;
            return Ok(nonce);
        }
    }

    Err("Could not retrieve nonce".to_string())
}

async fn query_eth_balance(address: &str) -> Result<String, String> {
    let rpc_url = "https://1rpc.io/eth";
    
    let balance_request = json!({
        "method": "eth_getBalance",
        "params": [address, "latest"],
        "jsonrpc": "2.0",
        "id": 1
    });

    let request = CanisterHttpRequestArgument {
        url: rpc_url.to_string(),
        method: HttpMethod::POST,
        body: Some(balance_request.to_string().as_bytes().to_vec()),
        max_response_bytes: Some(4096),
        transform: Some(TransformContext::from_name("transform".to_string(), to_vec(&()).unwrap())),
        headers: vec![
            HttpHeader {
                name: "Content-Type".to_string(),
                value: "application/json".to_string(),
            },
        ],
    };

    let (response,): (HttpResponse,) = http_request(request, 3_000_000_000)
        .await
        .map_err(|e| format!("HTTP request failed: {:?}", e))?;

    let response_body = String::from_utf8(response.body)
        .map_err(|e| format!("Failed to parse response body: {}", e))?;

    let response_json: Value = serde_json::from_str(&response_body)
        .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

    if let Some(result) = response_json.get("result") {
        if let Some(balance_hex) = result.as_str() {
            let balance_wei = u128::from_str_radix(balance_hex.strip_prefix("0x").unwrap_or(balance_hex), 16)
                .map_err(|_| "Failed to parse balance")?;
            let balance_eth = (balance_wei as f64) / 1e18;
            return Ok(format!("{:.6} ETH ({} wei)", balance_eth, balance_wei));
        }
    }

    Err("Could not retrieve balance".to_string())
}

// HTTP transform function
fn transform(raw: TransformArgs) -> HttpResponse {
    let headers = vec![
        HttpHeader {
            name: "Content-Security-Policy".to_string(),
            value: "default-src 'self'".to_string(),
        },
        HttpHeader {
            name: "Referrer-Policy".to_string(),
            value: "strict-origin".to_string(),
        },
    ];
    let mut sanitized = raw.response;
    sanitized.headers = headers;
    sanitized
}

// Public API functions
#[update]
pub async fn create_or_sign_eth_transaction_dynamic_gas(
    msg_id: String,
    to_address: String,
    amount_eth: String,
) -> Result<String, String> {
    let gas_price_wei = query_eth_gas_price().await?;
    let gas_price_gwei = (gas_price_wei.parse::<u128>().unwrap_or(3_000_000_000) as f64) / 1e9;
    
    create_or_sign_eth_transaction_with_gas_estimation(
        msg_id, 
        to_address, 
        amount_eth, 
        gas_price_gwei as u64
    ).await
}

pub async fn create_or_sign_eth_transaction(
    msg_id: String,
    to_address: String,
    amount_eth: String,
) -> Result<String, String> {
    create_or_sign_eth_transaction_dynamic_gas(msg_id, to_address, amount_eth).await
}

pub async fn get_current_gas_price() -> Result<String, String> {
    let gas_price_wei = query_eth_gas_price().await?;
    let gas_price_gwei = (gas_price_wei.parse::<u128>().unwrap_or(0) as f64) / 1e9;
    
    Ok(format!("Current network gas price: {:.2} Gwei ({} wei)", gas_price_gwei, gas_price_wei))
}

    #[update]
pub async fn get_eth_balance() -> Result<String, String> {
    let address = ETH_STATE.with(|state| {
        state.borrow().eth_key_info.as_ref()
            .map(|info| info.eth_address.clone())
    }).ok_or("Ethereum address not initialized")?;

    query_eth_balance(&address).await
}

#[update]
pub fn init_eth_multisig(signers: Vec<candid::Principal>, threshold: u32) -> Result<String, String> {
    if threshold == 0 || threshold > signers.len() as u32 {
        return Err("Invalid threshold value".to_string());
    }

    ETH_STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.signers = signers.clone();
        s.threshold = threshold;
    });

    Ok(format!("Ethereum Multisig initialized with {} signers and threshold {}", signers.len(), threshold))
}

#[query]
fn get_eth_transactions() -> Vec<(String, EthTransactionRecord)> {
    ETH_STATE.with(|state| {
        state.borrow().transactions.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    })
}

#[query]
pub async fn get_eth_address() -> Result<String, String> {
    ETH_STATE.with(|state| {
        state.borrow().eth_key_info.as_ref()
            .map(|info| info.eth_address.clone())
            .ok_or("Ethereum key info not initialized".to_string())
    })
}

#[query]
fn get_eth_signers_and_threshold() -> (Vec<candid::Principal>, u32) {
    ETH_STATE.with(|state| {
        let s = state.borrow();
        (s.signers.clone(), s.threshold)
    })
}

// Setup helper function
    pub async fn setup_identity_for_testing() -> Result<String, String> {
    let caller_principal = caller();
    
    let init_result = init_eth_multisig(vec![caller_principal], 1)?;
    let key_info = get_eth_key_info().await?;
    let balance = get_eth_balance().await.unwrap_or("0 ETH".to_string());
    let gas_price = get_current_gas_price().await.unwrap_or("Unknown".to_string());
    
    Ok(format!(
        "Identity Setup Complete!\nPrincipal: {}\nEthereum Address: {}\nCurrent Balance: {}\nNetwork Gas Price: {}\nMultisig: {}",
        caller_principal.to_text(),
        key_info.eth_address,
        balance,
        gas_price,
        init_result
    ))
}