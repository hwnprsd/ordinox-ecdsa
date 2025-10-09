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
use sha2::{Sha256, Sha512, Digest};
use hex;
use ripemd::{Ripemd160, Digest as RipemdDigest};

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct XrpTransaction {
    pub account: String,
    pub destination: String,
    pub amount: String,
    pub fee: String,
    pub flags: u32,
    pub sequence: u32,
    pub destination_tag: Option<u32>,
    pub signing_pub_key: String,
    pub txn_signature: Option<String>,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct XrpTransactionRecord {
    pub id: String,
    pub transaction: XrpTransaction,
    pub signers: Vec<candid::Principal>,
    pub executed: bool,
    pub tx_hash: Option<String>,
    pub timestamp: u64,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct XrpKeyInfo {
    pub public_key: Vec<u8>,
    pub compressed_key: Vec<u8>,
    pub xrp_address: String,
    pub derivation_path: Vec<Vec<u8>>,
}

thread_local! {
    static XRP_STATE: std::cell::RefCell<XrpState> = std::cell::RefCell::new(XrpState::default());
}

#[derive(Default)]
struct XrpState {
    transactions: HashMap<String, XrpTransactionRecord>,
    signers: Vec<candid::Principal>,
    threshold: u32,
    xrp_key_info: Option<XrpKeyInfo>,
    sequence_number: u32,
}

fn compress_secp256k1_public_key(uncompressed_key: &[u8]) -> Result<Vec<u8>, String> {
    if uncompressed_key.len() != 65 {
        return Err("Invalid uncompressed secp256k1 public key length, expected 65 bytes".to_string());
    }
    if uncompressed_key[0] != 0x04 {
        return Err("Invalid uncompressed public key format, must start with 0x04".to_string());
    }
    let x = &uncompressed_key[1..33];
    let y = &uncompressed_key[33..65];
    let y_is_even = y[31] % 2 == 0;
    let mut compressed = Vec::with_capacity(33);
    compressed.push(if y_is_even { 0x02 } else { 0x03 });
    compressed.extend_from_slice(x);
    Ok(compressed)
}

fn derive_xrp_address_from_secp256k1(public_key: &[u8]) -> Result<String, String> {
    let compressed_key = match public_key.len() {
        33 => public_key.to_vec(),
        65 => compress_secp256k1_public_key(public_key)?,
        _ => return Err(format!("Invalid secp256k1 public key length: {} bytes", public_key.len())),
    };
    let mut hasher = Sha256::new();
    hasher.update(&compressed_key);
    let sha256_result = hasher.finalize();
    let mut ripemd = Ripemd160::new();
    ripemd.update(sha256_result);
    let ripemd_result = ripemd.finalize();
    let mut versioned = vec![0x00];
    versioned.extend_from_slice(&ripemd_result);
    let mut hasher1 = Sha256::new();
    hasher1.update(&versioned);
    let hash1 = hasher1.finalize();
    let mut hasher2 = Sha256::new();
    hasher2.update(hash1);
    let hash2 = hasher2.finalize();
    let checksum = &hash2[0..4];
    let mut final_bytes = versioned;
    final_bytes.extend_from_slice(checksum);
    let address = base58_encode_xrp(&final_bytes);
    Ok(address)
}

fn base58_encode_xrp(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz";
    if data.is_empty() {
        return String::new();
    }
    let mut leading_zeros = 0;
    for &byte in data {
        if byte == 0 {
            leading_zeros += 1;
        } else {
            break;
        }
    }
    let mut digits = vec![0u32; data.len() * 138 / 100 + 1];
    let mut digits_len = 1;
    for &byte in data {
        let mut carry = byte as u32;
        for i in 0..digits_len {
            carry += digits[i] * 256;
            digits[i] = carry % 58;
            carry /= 58;
        }
        while carry > 0 {
            digits[digits_len] = carry % 58;
            digits_len += 1;
            carry /= 58;
        }
    }
    let mut result = Vec::new();
    for _ in 0..leading_zeros {
        result.push(b'r');
    }
    for i in (0..digits_len).rev() {
        if digits[i] < 58 {
            result.push(ALPHABET[digits[i] as usize]);
        }
    }
    String::from_utf8(result).unwrap_or_default()
}

fn base58_decode(input: &str) -> Result<Vec<u8>, String> {
    const ALPHABET: &[u8] = b"rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz";
    if input.is_empty() {
        return Ok(Vec::new());
    }
    let leading_zeros = input.chars().take_while(|&c| c == 'r').count();
    let mut num: Vec<u8> = vec![0];
    for ch in input.chars() {
        let digit_value = ALPHABET.iter().position(|&x| x == (ch as u8))
            .ok_or_else(|| format!("Invalid character '{}' in base58 string", ch))?;
        let mut carry = 0u32;
        for byte in num.iter_mut().rev() {
            let val = (*byte as u32) * 58 + carry;
            *byte = (val % 256) as u8;
            carry = val / 256;
        }
        while carry > 0 {
            num.insert(0, (carry % 256) as u8);
            carry /= 256;
        }
        let mut carry = digit_value as u32;
        for byte in num.iter_mut().rev() {
            let val = (*byte as u32) + carry;
            *byte = (val % 256) as u8;
            carry = val / 256;
        }
        while carry > 0 {
            num.insert(0, (carry % 256) as u8);
            carry /= 256;
        }
    }
    let mut result = vec![0u8; leading_zeros];
    result.extend_from_slice(&num);
    Ok(result)
}

fn decode_xrp_address_corrected(address: &str) -> Result<Vec<u8>, String> {
    if address.is_empty() || !address.starts_with('r') {
        return Err(format!("Invalid XRP address format: {}", address));
    }
    let decoded = base58_decode(address)
        .map_err(|e| format!("Base58 decode failed: {}", e))?;
    if decoded.len() != 25 {
        return Err(format!("Invalid decoded address length: {} bytes", decoded.len()));
    }
    let payload = &decoded[0..21];
    let checksum = &decoded[21..25];
    let mut hasher1 = Sha256::new();
    hasher1.update(payload);
    let hash1 = hasher1.finalize();
    let mut hasher2 = Sha256::new();
    hasher2.update(hash1);
    let hash2 = hasher2.finalize();
    let expected_checksum = &hash2[0..4];
    if checksum != expected_checksum {
        return Err("Address checksum validation failed".to_string());
    }
    Ok(decoded[1..21].to_vec())
}

#[update]
async fn get_xrp_key_info() -> Result<XrpKeyInfo, String> {
    let derivation_path = vec![b"xrp".to_vec(), caller().as_slice().to_vec()];
    let request = EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path: derivation_path.clone(),
        key_id: EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: "key_1".to_string(),
        },
    };
    let (response,): (EcdsaPublicKeyResponse,) = ecdsa_public_key(request)
        .await
        .map_err(|e| format!("Failed to get public key: {:?}", e))?;
    let public_key = response.public_key;
    let (uncompressed_key, compressed_key) = match public_key.len() {
        33 => (public_key.clone(), public_key.clone()),
        65 => (public_key.clone(), compress_secp256k1_public_key(&public_key)?),
        _ => return Err(format!("Unexpected public key length: {} bytes", public_key.len())),
    };
    let xrp_address = derive_xrp_address_from_secp256k1(&compressed_key)?;
    let key_info = XrpKeyInfo {
        public_key: uncompressed_key,
        compressed_key,
        xrp_address: xrp_address.clone(),
        derivation_path,
    };
    XRP_STATE.with(|state| {
        state.borrow_mut().xrp_key_info = Some(key_info.clone());
    });
    Ok(key_info)
}

fn convert_ecdsa_to_xrp_signature(ecdsa_signature: &[u8]) -> Result<Vec<u8>, String> {
    if ecdsa_signature.len() != 64 {
        return Err("Invalid ECDSA signature length, expected 64 bytes".to_string());
    }
    let r = &ecdsa_signature[0..32];
    let s = &ecdsa_signature[32..64];
    const CURVE_ORDER: [u8; 32] = [
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE,
        0xBA, 0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B,
        0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36, 0x41, 0x41
    ];
    const CURVE_ORDER_HALF: [u8; 32] = [
        0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0x5D, 0x57, 0x6E, 0x73, 0x57, 0xA4, 0x50, 0x1D,
        0xDF, 0xE9, 0x2F, 0x46, 0x68, 0x1B, 0x20, 0xA0
    ];
    if r == [0; 32] || s == [0; 32] {
        return Err("Invalid ECDSA signature: r or s is zero".to_string());
    }
    if r >= &CURVE_ORDER[..] || s >= &CURVE_ORDER[..] {
        return Err("Invalid ECDSA signature: r or s exceeds curve order".to_string());
    }
    let mut s_canonical = s.to_vec();
    if s > &CURVE_ORDER_HALF[..] {
        let mut result = [0u8; 32];
        let mut borrow = 0u64;
        for i in (0..32).rev() {
            let curve_byte = CURVE_ORDER[i] as u64;
            let s_byte = s[i] as u64;
            let diff = curve_byte.wrapping_sub(s_byte).wrapping_sub(borrow);
            result[i] = (diff & 0xFF) as u8;
            borrow = if diff > 0xFF { 1 } else { 0 };
        }
        s_canonical = result.to_vec();
    }
    let mut der_signature = Vec::new();
    let mut content = Vec::new();
    fn trim_leading_zeros(data: &[u8]) -> &[u8] {
        let first_non_zero = data.iter().position(|&x| x != 0).unwrap_or(data.len());
        if first_non_zero == data.len() {
            &data[data.len() - 1..]
        } else {
            &data[first_non_zero..]
        }
    }
    content.push(0x02);
    let r_trimmed = trim_leading_zeros(r);
    if r_trimmed.is_empty() || r_trimmed[0] >= 0x80 {
        content.push((r_trimmed.len() + 1) as u8);
        content.push(0x00);
        content.extend_from_slice(r_trimmed);
    } else {
        content.push(r_trimmed.len() as u8);
        content.extend_from_slice(r_trimmed);
    }
    content.push(0x02);
    let s_trimmed = trim_leading_zeros(&s_canonical);
    if s_trimmed.is_empty() || s_trimmed[0] >= 0x80 {
        content.push((s_trimmed.len() + 1) as u8);
        content.push(0x00);
        content.extend_from_slice(s_trimmed);
    } else {
        content.push(s_trimmed.len() as u8);
        content.extend_from_slice(s_trimmed);
    }
    der_signature.push(0x30);
    if content.len() > 0x7F {
        return Err("DER content length exceeds 127 bytes, unsupported".to_string());
    }
    der_signature.push(content.len() as u8);
    der_signature.extend_from_slice(&content);
    Ok(der_signature)
}

fn create_xrp_transaction_blob(tx: &XrpTransaction) -> Result<String, String> {
    let mut encoded = Vec::new();

    // Validate inputs
    if tx.account.is_empty() || tx.destination.is_empty() {
        return Err("Account or destination address cannot be empty".to_string());
    }
    if tx.sequence == 0 {
        return Err("Sequence cannot be zero".to_string());
    }
    if tx.flags & 0x80000000 == 0 {
        return Err("Flags must include tfFullyCanonicalSig (0x80000000)".to_string());
    }

    // CORRECT field ordering based on working blob analysis:

    // 1. TransactionType (UInt8, type=1, field=1) - CORRECTED encoding
    encoded.push(0x11); // Field ID
    encoded.push(0x12); // Payment transaction type (NO 0x00 separator)

    // 2. Flags (UInt32, type=2, field=2)
    encoded.push(0x22);
    encoded.extend_from_slice(&tx.flags.to_be_bytes());

    // 3. Sequence (UInt32, type=2, field=4)
    encoded.push(0x24);
    encoded.extend_from_slice(&tx.sequence.to_be_bytes());

    // 4. DestinationTag (UInt32, type=2, field=14, optional)
    if let Some(tag) = tx.destination_tag {
        encoded.push(0x2E);
        encoded.extend_from_slice(&tag.to_be_bytes());
    }

    // 5. Account (AccountID, type=8, field=1) - MOVED UP before Amount
    encoded.push(0x81);
    let account_bytes = decode_xrp_address_corrected(&tx.account)?;
    if account_bytes.len() != 20 {
        return Err(format!("Invalid account address: expected 20 bytes, got {}", account_bytes.len()));
    }
    encoded.extend_from_slice(&account_bytes);

    // 6. Destination (AccountID, type=8, field=3) - MOVED UP before Amount
    encoded.push(0x83);
    let dest_bytes = decode_xrp_address_corrected(&tx.destination)?;
    if dest_bytes.len() != 20 {
        return Err(format!("Invalid destination address: expected 20 bytes, got {}", dest_bytes.len()));
    }
    encoded.extend_from_slice(&dest_bytes);

    // 7. Amount (UInt64, type=6, field=1) - MOVED DOWN after Account fields
    encoded.push(0x61);
    let amount: u64 = tx.amount.parse().map_err(|e| format!("Invalid amount format: {}", e))?;
    if amount == 0 {
        return Err("Amount cannot be zero".to_string());
    }
    if amount > 100_000_000_000_000_000 {
        return Err("Amount exceeds XRP Ledger limit (100B XRP)".to_string());
    }
    // CRITICAL: Based on working blob, use RAW amount (no positive bit)
    encoded.extend_from_slice(&amount.to_be_bytes());

    // 8. Fee (UInt64, type=6, field=8) - MOVED DOWN after Account fields
    encoded.push(0x68);
    let fee: u64 = tx.fee.parse().map_err(|e| format!("Invalid fee format: {}", e))?;
    if fee == 0 {
        return Err("Fee cannot be zero".to_string());
    }
    // CRITICAL: Based on working blob, use RAW fee (no positive bit)
    encoded.extend_from_slice(&fee.to_be_bytes());

    // 9. SigningPubKey (Blob, type=7, field=3)
    encoded.push(0x73);
    let pub_key_bytes = hex::decode(&tx.signing_pub_key).map_err(|_| "Invalid public key hex")?;
    if pub_key_bytes.len() != 33 || (pub_key_bytes[0] != 0x02 && pub_key_bytes[0] != 0x03) {
        return Err(format!("Invalid public key: must be 33-byte compressed key, got {} bytes", pub_key_bytes.len()));
    }
    encoded.push(33);
    encoded.extend_from_slice(&pub_key_bytes);

    // 10. TxnSignature (Blob, type=7, field=6, optional)
    if let Some(signature) = &tx.txn_signature {
        encoded.push(0x76);
        let sig_bytes = hex::decode(signature).map_err(|_| "Invalid signature hex")?;
        if sig_bytes.len() < 68 || sig_bytes.len() > 72 {
            return Err(format!("Invalid signature length: {} bytes (expected 68-72)", sig_bytes.len()));
        }
        encoded.push(sig_bytes.len() as u8);
        encoded.extend_from_slice(&sig_bytes);
    }

    let blob = hex::encode(&encoded).to_uppercase();
    ic_cdk::println!("Corrected transaction blob: {} ({} bytes)", blob, encoded.len());
    Ok(blob)
}

fn create_xrp_transaction_blob_unsigned(tx: &XrpTransaction) -> Result<String, String> {
    let mut encoded = Vec::new();

    // Same validation and ordering as above, minus TxnSignature

    if tx.account.is_empty() || tx.destination.is_empty() {
        return Err("Account or destination address cannot be empty".to_string());
    }
    if tx.sequence == 0 {
        return Err("Sequence cannot be zero".to_string());
    }
    if tx.flags & 0x80000000 == 0 {
        return Err("Flags must include tfFullyCanonicalSig (0x80000000)".to_string());
    }

    // CORRECTED field ordering:

    // 1. TransactionType - FIXED encoding
    encoded.push(0x11);
    encoded.push(0x12); // No 0x00 separator

    // 2. Flags
    encoded.push(0x22);
    encoded.extend_from_slice(&tx.flags.to_be_bytes());

    // 3. Sequence
    encoded.push(0x24);
    encoded.extend_from_slice(&tx.sequence.to_be_bytes());

    // 4. DestinationTag (optional)
    if let Some(tag) = tx.destination_tag {
        encoded.push(0x2E);
        encoded.extend_from_slice(&tag.to_be_bytes());
    }

    // 5. Account - MOVED UP
    encoded.push(0x81);
    let account_bytes = decode_xrp_address_corrected(&tx.account)?;
    if account_bytes.len() != 20 {
        return Err(format!("Invalid account address: expected 20 bytes, got {}", account_bytes.len()));
    }
    encoded.extend_from_slice(&account_bytes);

    // 6. Destination - MOVED UP
    encoded.push(0x83);
    let dest_bytes = decode_xrp_address_corrected(&tx.destination)?;
    if dest_bytes.len() != 20 {
        return Err(format!("Invalid destination address: expected 20 bytes, got {}", dest_bytes.len()));
    }
    encoded.extend_from_slice(&dest_bytes);

    // 7. Amount - MOVED DOWN, no positive bit
    encoded.push(0x61);
    let amount: u64 = tx.amount.parse().map_err(|e| format!("Invalid amount format: {}", e))?;
    if amount == 0 {
        return Err("Amount cannot be zero".to_string());
    }
    if amount > 100_000_000_000_000_000 {
        return Err("Amount exceeds XRP Ledger limit (100B XRP)".to_string());
    }
    encoded.extend_from_slice(&amount.to_be_bytes());

    // 8. Fee - MOVED DOWN, no positive bit
    encoded.push(0x68);
    let fee: u64 = tx.fee.parse().map_err(|e| format!("Invalid fee format: {}", e))?;
    if fee == 0 {
        return Err("Fee cannot be zero".to_string());
    }
    encoded.extend_from_slice(&fee.to_be_bytes());

    // 9. SigningPubKey
    encoded.push(0x73);
    let pub_key_bytes = hex::decode(&tx.signing_pub_key).map_err(|_| "Invalid public key hex")?;
    if pub_key_bytes.len() != 33 || (pub_key_bytes[0] != 0x02 && pub_key_bytes[0] != 0x03) {
        return Err(format!("Invalid public key: must be 33-byte compressed key, got {} bytes", pub_key_bytes.len()));
    }
    encoded.push(33);
    encoded.extend_from_slice(&pub_key_bytes);

    let blob = hex::encode(&encoded).to_uppercase();
    ic_cdk::println!("Corrected unsigned transaction blob: {} ({} bytes)", blob, encoded.len());
    Ok(blob)
}

fn create_xrp_transaction_hash(tx: &XrpTransaction) -> Result<Vec<u8>, String> {
    let mut signing_data = Vec::new();
    signing_data.extend_from_slice(b"STX\x00");
    let unsigned_blob = create_xrp_transaction_blob_unsigned(tx)?;
    let unsigned_bytes = hex::decode(&unsigned_blob)
        .map_err(|_| "Failed to decode unsigned transaction blob")?;
    signing_data.extend_from_slice(&unsigned_bytes);
    let mut hasher = Sha512::new();
    hasher.update(&signing_data);
    let hash = hasher.finalize();
    Ok(hash[0..32].to_vec())
}

#[update]
pub async fn create_or_sign_xrp_transaction(
    msg_id: String,
    to_address: String,
    amount: String,
) -> Result<String, String> {
    let caller = caller();

    if msg_id.trim().is_empty() {
        return Err("Transaction ID cannot be empty".to_string());
    }
    if to_address.trim().is_empty() {
        return Err("Destination address cannot be empty".to_string());
    }

    let (is_authorized, key_info, threshold) = XRP_STATE.with(|state| {
        let s = state.borrow();
        let is_authorized = s.signers.contains(&caller);
        let key_info = s.xrp_key_info.clone();
        (is_authorized, key_info, s.threshold)
    });

    if !is_authorized {
        return Err("Caller is not an authorized signer".to_string());
    }

    let key_info = key_info.ok_or("XRP key info not initialized. Call get_xrp_key_info first")?;

    let amount_xrp: f64 = amount.parse().map_err(|_| "Invalid amount format")?;
    if amount_xrp <= 0.0 {
        return Err("Amount must be greater than 0".to_string());
    }
    let amount_drops = ((amount_xrp * 1_000_000.0) as u64).to_string();

    let fee_drops = query_xrp_fee().await.unwrap_or("12".to_string());

    let (transaction_exists, message_exists) = XRP_STATE.with(|state| {
        let s = state.borrow();
        let exists = s.transactions.contains_key(&msg_id);
        (exists, exists)
    });

    if threshold == 1 {
        if message_exists {
            return Err("Transaction already executed".to_string());
        }

        let current_sequence = query_xrp_account_info(&key_info.xrp_address)
            .await
            .map_err(|e| format!("Failed to get sequence number: {}", e))?;

        let transaction = XrpTransaction {
            account: key_info.xrp_address.clone(),
            destination: to_address,
            amount: amount_drops,
            fee: fee_drops,
            flags: 0x80000000,
            sequence: current_sequence,
            destination_tag: None,
            signing_pub_key: hex::encode(&key_info.compressed_key),
            txn_signature: None,
        };

        let tx_record = XrpTransactionRecord {
            id: msg_id.clone(),
            transaction,
            signers: vec![caller],
            executed: false,
            tx_hash: None,
            timestamp: ic_cdk::api::time(),
        };

        XRP_STATE.with(|state| {
            state.borrow_mut().transactions.insert(msg_id.clone(), tx_record);
        });

        execute_xrp_transaction(msg_id.clone()).await
    } else {
        if message_exists {
            let should_execute = XRP_STATE.with(|state| {
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
                execute_xrp_transaction(msg_id.clone()).await
            } else {
                let signer_count = XRP_STATE.with(|s|
                    s.borrow().transactions.get(&msg_id).unwrap().signers.len()
                );
                Ok(format!("Transaction {} signed. {}/{} signatures",
                    msg_id, signer_count, threshold))
            }
        } else {
            let current_sequence = query_xrp_account_info(&key_info.xrp_address)
                .await
                .map_err(|e| format!("Failed to get sequence number: {}", e))?;

            let transaction = XrpTransaction {
                account: key_info.xrp_address.clone(),
                destination: to_address,
                amount: amount_drops,
                fee: fee_drops,
                flags: 0x80000000,
                sequence: current_sequence,
                destination_tag: None,
                signing_pub_key: hex::encode(&key_info.compressed_key),
                txn_signature: None,
            };

            let tx_record = XrpTransactionRecord {
                id: msg_id.clone(),
                transaction,
                signers: vec![caller],
                executed: false,
                tx_hash: None,
                timestamp: ic_cdk::api::time(),
            };

            XRP_STATE.with(|state| {
                state.borrow_mut().transactions.insert(msg_id.clone(), tx_record);
            });

            Ok(format!("Transaction {} created. 1/{} signatures", msg_id, threshold))
        }
    }
}

async fn execute_xrp_transaction(transaction_id: String) -> Result<String, String> {
    let (mut transaction, key_info) = XRP_STATE.with(|state| {
        let s = state.borrow();
        if let Some(tx_record) = s.transactions.get(&transaction_id) {
            let key_info = s.xrp_key_info.clone().ok_or("XRP key info not available")?;
            Ok((tx_record.transaction.clone(), key_info))
        } else {
            Err("Transaction not found".to_string())
        }
    })?;

    let tx_hash = create_xrp_transaction_hash(&transaction)?;
    ic_cdk::println!("Transaction hash to sign: {}", hex::encode(&tx_hash));

    let request = SignWithEcdsaArgument {
        message_hash: tx_hash,
        derivation_path: key_info.derivation_path.clone(),
        key_id: EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: "key_1".to_string(),
        },
    };

    let (response,): (SignWithEcdsaResponse,) = sign_with_ecdsa(request)
        .await
        .map_err(|e| format!("Failed to sign transaction: {:?}", e))?;

    let xrp_signature = convert_ecdsa_to_xrp_signature(&response.signature)?;
    transaction.txn_signature = Some(hex::encode(&xrp_signature));
    ic_cdk::println!("Signature (DER): {}", hex::encode(&xrp_signature));

    let tx_blob = create_xrp_transaction_blob(&transaction)?;
    ic_cdk::println!("📤 Submitting transaction blob: {}", tx_blob);

    let submission_result = submit_xrp_transaction_to_network(&tx_blob).await?;

    XRP_STATE.with(|state| {
        let mut s = state.borrow_mut();
        if let Some(tx_record) = s.transactions.get_mut(&transaction_id) {
            tx_record.executed = true;
            tx_record.tx_hash = Some(submission_result.clone());
            tx_record.transaction.txn_signature = transaction.txn_signature.clone();
            s.sequence_number = transaction.sequence + 1;
        }
    });

    Ok(format!("Transaction {} executed successfully. Hash: {}", transaction_id, submission_result))
}

async fn submit_xrp_transaction_to_network(tx_blob: &str) -> Result<String, String> {
    let rpc_endpoints = vec![
        "https://s1.ripple.com:51234",
        "https://s2.ripple.com:51234",
        "https://xrplcluster.com",
    ];
    let mut last_error = String::new();
    for rpc_url in rpc_endpoints {
        match submit_to_single_endpoint(rpc_url, tx_blob).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = format!("Failed with {}: {}", rpc_url, e);
                ic_cdk::println!("Submission error: {}", last_error);
                continue;
            }
        }
    }
    Err(format!("All RPC endpoints failed. Last error: {}", last_error))
}

async fn submit_to_single_endpoint(rpc_url: &str, tx_blob: &str) -> Result<String, String> {
    let submit_request = json!({
        "method": "submit",
        "params": [{
            "tx_blob": tx_blob,
            "ledger_index": "current"
        }],
        "jsonrpc": "2.0",
        "id": 1
    });

    let request_body = submit_request.to_string();
    let request_headers = vec![
        HttpHeader {
            name: "Content-Type".to_string(),
            value: "application/json".to_string(),
        },
        HttpHeader {
            name: "Accept".to_string(),
            value: "application/json".to_string(),
        },
    ];

    let request = CanisterHttpRequestArgument {
        url: rpc_url.to_string(),
        method: HttpMethod::POST,
        body: Some(request_body.as_bytes().to_vec()),
        max_response_bytes: Some(8192),
        transform: Some(TransformContext::from_name("transform".to_string(), to_vec(&()).unwrap())),
        headers: request_headers,
    };

    let (response,): (HttpResponse,) = http_request(request, 5_000_000_000)
        .await
        .map_err(|e| format!("HTTP request failed: {:?}", e))?;

    let response_body = String::from_utf8(response.body)
        .map_err(|e| format!("Failed to parse response body: {}", e))?;

    ic_cdk::println!("RPC Response from {}: {}", rpc_url, response_body);

    let response_json: Value = serde_json::from_str(&response_body)
        .map_err(|e| format!("Failed to parse JSON response: {}. Raw response: {}", e, response_body))?;

    if let Some(error) = response_json.get("error") {
        return Err(format!("RPC Error: {}", error));
    }

    if let Some(result) = response_json.get("result") {
        return handle_submit_result(result);
    }

    if response_json.get("engine_result").is_some() {
        return handle_submit_result(&response_json);
    }

    Err(format!("Unexpected response format. Full response: {}", response_body))
}

fn handle_submit_result(result: &Value) -> Result<String, String> {
    if let Some(engine_result) = result.get("engine_result") {
        let engine_result_str = engine_result.as_str().unwrap_or("");
        match engine_result_str {
            "tesSUCCESS" => {
                if let Some(tx_json) = result.get("tx_json") {
                    if let Some(hash) = tx_json.get("hash") {
                        return Ok(hash.as_str().unwrap_or("unknown").to_string());
                    }
                }
                if let Some(hash) = result.get("tx_id") {
                    return Ok(hash.as_str().unwrap_or("unknown").to_string());
                }
                Ok("Transaction submitted successfully (no hash returned)".to_string())
            },
            "terQUEUED" => Ok("Transaction queued for processing".to_string()),
            "tefPAST_SEQ" => Err("Transaction sequence number is too old".to_string()),
            "tefMAX_LEDGER" => Err("Transaction expired (ledger sequence too high)".to_string()),
            "tecUNFUNDED_PAYMENT" => Err("Insufficient XRP balance to complete payment".to_string()),
            "tecNO_DST_INSUF_XRP" => Err("Destination requires minimum XRP balance".to_string()),
            _ => {
                let error_message = result.get("engine_result_message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error");
                Err(format!("Transaction failed: {} - {}", engine_result_str, error_message))
            }
        }
    } else {
        if let Some(error_code) = result.get("error") {
            return Err(format!("Server error: {}", error_code));
        }
        if let Some(error_message) = result.get("error_message") {
            return Err(format!("Server error: {}", error_message));
        }
        Err(format!("Invalid response format: missing engine_result. Result: {}", result))
    }
}

#[query]
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

pub async fn query_xrp_account_info(address: &str) -> Result<u32, String> {
    let rpc_url = "https://s1.ripple.com:51234";
    let account_info_request = json!({
        "method": "account_info",
        "params": [{
            "account": address,
            "ledger_index": "current"
        }],
        "jsonrpc": "2.0",
        "id": 1
    });

    let request_body = account_info_request.to_string();
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

    ic_cdk::println!("Account info response: {}", response_body);

    let response_json: Value = serde_json::from_str(&response_body)
        .map_err(|e| format!("Failed to parse JSON response: {}. Raw response: {}", e, response_body))?;

    if let Some(error) = response_json.get("error") {
        return Err(format!("RPC Error: {}", error));
    }

    if let Some(result) = response_json.get("result") {
        if let Some(account_data) = result.get("account_data") {
            if let Some(sequence) = account_data.get("Sequence") {
                return Ok(sequence.as_u64().unwrap_or(0) as u32);
            }
        }
    }

    Err(format!("Could not retrieve account sequence. Response: {}", response_body))
}

async fn query_xrp_fee() -> Result<String, String> {
    let rpc_url = "https://s1.ripple.com:51234";
    let fee_request = json!({
        "method": "fee",
        "params": [{}],
        "jsonrpc": "2.0",
        "id": 1
    });

    let request_body = fee_request.to_string();
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

    ic_cdk::println!("Fee response: {}", response_body);

    let response_json: Value = serde_json::from_str(&response_body)
        .map_err(|e| format!("Failed to parse JSON response: {}. Raw response: {}", e, response_body))?;

    if let Some(error) = response_json.get("error") {
        return Err(format!("RPC Error: {}", error));
    }

    if let Some(result) = response_json.get("result") {
        if let Some(drops) = result.get("drops") {
            if let Some(base_fee) = drops.get("base_fee") {
                return Ok(base_fee.as_str().unwrap_or("12").to_string());
            }
        }
    }

    Err(format!("Could not retrieve fee. Response: {}", response_body))
}

#[update]
pub async fn get_xrp_balance() -> Result<String, String> {
    let address = get_xrp_address()?;
    let sequence = query_xrp_account_info(&address).await?;
    let balance_request = json!({
        "method": "account_info",
        "params": [{
            "account": address,
            "ledger_index": "current"
        }],
        "jsonrpc": "2.0",
        "id": 1
    });

    let request_body = balance_request.to_string();
    let request_headers = vec![
        HttpHeader {
            name: "Content-Type".to_string(),
            value: "application/json".to_string(),
        },
    ];

    let request = CanisterHttpRequestArgument {
        url: "https://s1.ripple.com:51234".to_string(),
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

    ic_cdk::println!("Balance response: {}", response_body);

    let response_json: Value = serde_json::from_str(&response_body)
        .map_err(|e| format!("Failed to parse JSON response: {}. Raw response: {}", e, response_body))?;

    if let Some(error) = response_json.get("error") {
        return Err(format!("RPC Error: {}", error));
    }

    if let Some(result) = response_json.get("result") {
        if let Some(account_data) = result.get("account_data") {
            if let Some(balance) = account_data.get("Balance") {
                let balance_drops = balance.as_str().unwrap_or("0");
                let balance_xrp = (balance_drops.parse::<u64>().unwrap_or(0) as f64) / 1_000_000.0;
                return Ok(format!("{} XRP ({} drops, sequence {})", balance_xrp, balance_drops, sequence));
            }
        }
    }

    Err(format!("Could not retrieve balance. Response: {}", response_body))
}

#[query]
fn get_xrp_transactions() -> Vec<(String, XrpTransactionRecord)> {
    XRP_STATE.with(|state| {
        state.borrow().transactions.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    })
}

#[query]
pub fn get_xrp_address() -> Result<String, String> {
    XRP_STATE.with(|state| {
        state.borrow().xrp_key_info.as_ref()
            .map(|info| info.xrp_address.clone())
            .ok_or("XRP key info not initialized".to_string())
    })
}

#[update]
pub fn init_xrp_multisig(signers: Vec<candid::Principal>, threshold: u32) -> Result<String, String> {
    if threshold == 0 || threshold > signers.len() as u32 {
        return Err("Invalid threshold value".to_string());
    }
    XRP_STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.signers = signers.clone();
        s.threshold = threshold;
    });
    Ok(format!("XRP Multisig initialized with {} signers and threshold {}", signers.len(), threshold))
}

#[query]
fn get_xrp_signers_and_threshold() -> (Vec<candid::Principal>, u32) {
    XRP_STATE.with(|state| {
        let s = state.borrow();
        (s.signers.clone(), s.threshold)
    })
}

