// use ic_cdk::api::management_canister::ecdsa::{
//     sign_with_ecdsa, EcdsaCurve, EcdsaKeyId, SignWithEcdsaArgument,
// };
// use ic_cdk::{query, update, caller};
// use serde::{Deserialize, Serialize};
// use std::collections::HashMap;
// use candid::{CandidType, Principal};
// use hex;
// use sha2::{Digest, Sha512};

// // XRP transaction message for multi-signing
// #[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
// pub struct XrpTransferMessage {
//     pub signers: Vec<Principal>,
//     pub signature: Option<String>,
//     pub account: String,
//     pub destination: String,
//     pub amount: u64, // in drops (1 XRP = 1,000,000 drops)
//     pub fee: u64,    // in drops
//     pub sequence: u32,
//     pub last_ledger_sequence: Option<u32>,
//     pub destination_tag: Option<u32>,
//     pub timestamp: u64,
// }

// impl XrpTransferMessage {
//     pub fn hash(&self) -> String {
//         let mut hasher = sha2::Sha256::new();
//         hasher.update(self.account.as_bytes());
//         hasher.update(self.destination.as_bytes());
//         hasher.update(&self.amount.to_be_bytes());
//         hasher.update(&self.fee.to_be_bytes());
//         hasher.update(&self.sequence.to_be_bytes());
//         if let Some(last_ledger) = self.last_ledger_sequence {
//             hasher.update(&last_ledger.to_be_bytes());
//         }
//         if let Some(dest_tag) = self.destination_tag {
//             hasher.update(&dest_tag.to_be_bytes());
//         }
//         hasher.update(&self.timestamp.to_be_bytes());
//         hex::encode(hasher.finalize())
//     }

//     pub fn encode_for_signing(&self) -> Vec<u8> {
//         // Create XRP transaction in canonical format for signing
//         let mut buffer = Vec::new();
        
//         // Transaction type - Payment (0x12)
//         buffer.extend_from_slice(&[0x12, 0x00]); // TTxnType + Payment
        
//         // Sequence (0x24)
//         buffer.extend_from_slice(&[0x24]);
//         buffer.extend_from_slice(&self.sequence.to_be_bytes());
        
//         // LastLedgerSequence (optional)
//         if let Some(last_ledger) = self.last_ledger_sequence {
//             buffer.extend_from_slice(&[0x1B]);
//             buffer.extend_from_slice(&last_ledger.to_be_bytes());
//         }
        
//         // DestinationTag (optional)
//         if let Some(dest_tag) = self.destination_tag {
//             buffer.extend_from_slice(&[0x2E]);
//             buffer.extend_from_slice(&dest_tag.to_be_bytes());
//         }
        
//         // Amount - XRP native format
//         buffer.extend_from_slice(&[0x61]);
//         let amount_bytes = (0x4000000000000000u64 | self.amount).to_be_bytes();
//         buffer.extend_from_slice(&amount_bytes);
        
//         // Fee
//         buffer.extend_from_slice(&[0x68]);
//         let fee_bytes = (0x4000000000000000u64 | self.fee).to_be_bytes();
//         buffer.extend_from_slice(&fee_bytes);
        
//         // Account (simplified - using first 20 bytes of account string hash)
//         buffer.extend_from_slice(&[0x81, 0x14]); // TAccount + length
//         let mut hasher = sha2::Sha256::new();
//         hasher.update(self.account.as_bytes());
//         let account_hash = hasher.finalize();
//         buffer.extend_from_slice(&account_hash[0..20]);
        
//         // Destination (simplified - using first 20 bytes of destination string hash)
//         buffer.extend_from_slice(&[0x83, 0x14]); // TDestination + length
//         let mut hasher = sha2::Sha256::new();
//         hasher.update(self.destination.as_bytes());
//         let dest_hash = hasher.finalize();
//         buffer.extend_from_slice(&dest_hash[0..20]);
        
//         buffer
//     }
// }

// // State management
// #[derive(Default, CandidType, Serialize, Deserialize)]
// struct State {
//     signers: Vec<Principal>,
//     threshold: u32,
//     messages: HashMap<String, XrpTransferMessage>,
// }

// thread_local! {
//     static STATE: std::cell::RefCell<State> = std::cell::RefCell::new(State::default());
// }

// // Initialize the canister with signers and threshold
// #[update]
// fn init_multisig(signers: Vec<Principal>, threshold: u32) -> Result<String, String> {
//     if threshold == 0 || threshold as usize > signers.len() {
//         return Err("Invalid threshold".to_string());
//     }

//     STATE.with(|state| {
//         let mut s = state.borrow_mut();
//         s.signers = signers;
//         s.threshold = threshold;
//     });

//     Ok("Multi-signature wallet initialized".to_string())
// }

// // Main function to create or sign XRP transaction
// #[update]
// async fn create_or_sign_xrp_message(
//     account: String,
//     destination: String,
//     amount: u64,
//     fee: u64,
//     sequence: u32,
//     last_ledger_sequence: Option<u32>,
//     destination_tag: Option<u32>,
// ) -> Result<String, String> {
//     let caller = caller();

//     let mut msg = XrpTransferMessage {
//         signers: vec![],
//         signature: None,
//         account,
//         destination,
//         amount,
//         fee,
//         sequence,
//         last_ledger_sequence,
//         destination_tag,
//         timestamp: ic_cdk::api::time(),
//     };

//     let msg_id = msg.hash();

//     // Check if caller is authorized and if message exists
//     let (is_authorized, message_exists, threshold) = STATE.with(|state| {
//         let s = state.borrow();
//         let is_authorized = s.signers.contains(&caller);
//         let message_exists = s.messages.contains_key(&msg_id);
//         (is_authorized, message_exists, s.threshold)
//     });

//     if !is_authorized {
//         return Err("Caller is not an authorized signer".to_string());
//     }

//     if message_exists {
//         let should_sign = STATE.with(|state| {
//             let mut s = state.borrow_mut();
//             let message = s.messages.get_mut(&msg_id).unwrap();
//             if !message.signers.contains(&caller) {
//                 message.signers.push(caller);
//             }
//             message.signers.len() as u32 >= threshold && message.signature.is_none()
//         });

//         if should_sign {
//             sign_xrp_message(msg_id.clone()).await
//         } else {
//             Ok(msg_id)
//         }
//     } else {
//         STATE.with(|state| {
//             let mut s = state.borrow_mut();
//             msg.signers.push(caller);
//             s.messages.insert(msg_id.clone(), msg);
//             Ok(msg_id)
//         })
//     }
// }

// // Sign the XRP transaction
// async fn sign_xrp_message(msg_id: String) -> Result<String, String> {
//     let message = STATE
//         .with(|state| state.borrow().messages.get(&msg_id).cloned())
//         .ok_or("Message not found".to_string())?;

//     let tx_blob = message.encode_for_signing();
    
//     // Create XRP signing hash (SHA-512 half with signing prefix)
//     let signing_hash = create_xrp_signing_hash(&tx_blob)?;

//     // Sign with ECDSA using IC's threshold ECDSA
//     let signature = sign_xrp_transaction(&signing_hash).await?;

//     // Create the complete signed transaction blob
//     let signed_tx_blob = create_signed_transaction_blob(&tx_blob, &signature)?;

//     STATE.with(|state| {
//         let mut s = state.borrow_mut();
//         if let Some(msg) = s.messages.get_mut(&msg_id) {
//             msg.signature = Some(hex::encode(signed_tx_blob));
//         }
//     });

//     Ok(msg_id)
// }

// // Create XRP signing hash
// fn create_xrp_signing_hash(tx_blob: &[u8]) -> Result<Vec<u8>, String> {
//     // XRP signing prefix
//     const SIGNING_PREFIX: [u8; 4] = [0x53, 0x54, 0x58, 0x00]; // "STX\0"
    
//     let mut hasher = Sha512::new();
//     hasher.update(&SIGNING_PREFIX);
//     hasher.update(tx_blob);
//     let hash = hasher.finalize();
    
//     // Return first 32 bytes
//     Ok(hash[0..32].to_vec())
// }

// // Sign transaction using IC's ECDSA
// async fn sign_xrp_transaction(message_hash: &[u8]) -> Result<Vec<u8>, String> {
//     let key_id = EcdsaKeyId {
//         curve: EcdsaCurve::Secp256k1,
//         name: "secp256k1".to_string(),
//     };

//     let request = SignWithEcdsaArgument {
//         message_hash: message_hash.to_vec(),
//         derivation_path: vec![b"xrp-multisig".to_vec()],
//         key_id,
//     };

//     let (response,) = sign_with_ecdsa(request)
//         .await
//         .map_err(|e| format!("ECDSA signing failed: {:?}", e))?;

//     Ok(response.signature)
// }

// // Create signed transaction blob
// fn create_signed_transaction_blob(tx_blob: &[u8], signature: &[u8]) -> Result<Vec<u8>, String> {
//     let mut signed_blob = Vec::new();
    
//     // Add transaction signature field (0x76)
//     signed_blob.extend_from_slice(&[0x76]); // TTxnSignature
//     signed_blob.push(signature.len() as u8); // signature length
//     signed_blob.extend_from_slice(signature);
    
//     // Add the original transaction blob
//     signed_blob.extend_from_slice(tx_blob);
    
//     Ok(signed_blob)
// }

// // Query functions
// #[query]
// fn get_signature(msg_id: String) -> String {
//     STATE.with(|state| {
//         state
//             .borrow()
//             .messages
//             .get(&msg_id)
//             .map(|m| m.signature.clone().unwrap_or_else(|| "msg found but not signed".to_string()))
//             .unwrap_or_else(|| "no msg found with id".to_string())
//     })
// }

// #[query]
// fn get_message(msg_id: String) -> Option<XrpTransferMessage> {
//     STATE.with(|state| {
//         state.borrow().messages.get(&msg_id).cloned()
//     })
// }

// #[query]
// fn get_pending_messages() -> Vec<(String, XrpTransferMessage)> {
//     STATE.with(|state| {
//         state
//             .borrow()
//             .messages
//             .iter()
//             .filter(|(_, msg)| msg.signature.is_none())
//             .map(|(id, msg)| (id.clone(), msg.clone()))
//             .collect()
//     })
// }

// #[query]
// fn get_signers_and_threshold() -> (Vec<Principal>, u32) {
//     STATE.with(|state| {
//         let s = state.borrow();
//         (s.signers.clone(), s.threshold)
//     })
// }

// #[query]
// fn get_message_signers(msg_id: String) -> Vec<Principal> {
//     STATE.with(|state| {
//         state
//             .borrow()
//             .messages
//             .get(&msg_id)
//             .map(|m| m.signers.clone())
//             .unwrap_or_default()
//     })
// }

// // Utility function to estimate XRP fees
// #[query]
// fn get_base_fee() -> u64 {
//     // Base fee in drops (typically 10-12 drops)
//     10
// }

// // Helper function to convert XRP to drops
// #[query]
// fn xrp_to_drops(xrp_amount: f64) -> u64 {
//     (xrp_amount * 1_000_000.0) as u64
// }

// // Helper function to convert drops to XRP
// #[query]
// fn drops_to_xrp(drops: u64) -> f64 {
//     drops as f64 / 1_000_000.0
// }

// // Validate XRP address format (simplified)
// #[query]
// fn is_valid_xrp_address(address: String) -> bool {
//     address.starts_with('r') && address.len() >= 25 && address.len() <= 35
// }

// // Add HTTP outcall functionality for submitting to XRP ledger
// use ic_cdk::api::management_canister::http_request::{
//     http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod, HttpResponse,
// };

// // Submit signed transaction to XRP ledger
// #[update]
// async fn submit_to_xrp_ledger(msg_id: String, rpc_url: Option<String>) -> Result<String, String> {
//     // Get the signed transaction
//     let signed_tx_hex = STATE.with(|state| {
//         state
//             .borrow()
//             .messages
//             .get(&msg_id)
//             .and_then(|m| m.signature.clone())
//     }).ok_or("Transaction not found or not signed")?;

//     // Default to mainnet RPC if not provided
//     let url = rpc_url.unwrap_or_else(|| "https://xrplcluster.com".to_string());

//     // Create JSON-RPC request
//     let request_body = format!(
//         r#"{{
//             "method": "submit",
//             "params": [{{
//                 "tx_blob": "{}"
//             }}]
//         }}"#,
//         signed_tx_hex
//     );

//     let request = CanisterHttpRequestArgument {
//         url,
//         method: HttpMethod::POST,
//         body: Some(request_body.into_bytes()),
//         max_response_bytes: Some(2048),
//         transform: None,
//         headers: vec![
//             HttpHeader {
//                 name: "Content-Type".to_string(),
//                 value: "application/json".to_string(),
//             },
//         ],
//     };

//     match http_request(request, 25_000_000_000).await {
//         Ok((response,)) => {
//             let body = String::from_utf8(response.body)
//                 .map_err(|_| "Invalid response body")?;
            
//             // Parse response to check for success
//             if response.status == 200u8 {
//                 Ok(body)
//             } else {
//                 Err(format!("XRP ledger error: {}", body))
//             }
//         }
//         Err((r, m)) => Err(format!("HTTP request failed: {:?} {}", r, m)),
//     }
// }

// // Auto-submit version: signs AND submits in one call
// #[update]
// async fn create_sign_and_submit_xrp_transaction(
//     account: String,
//     destination: String,
//     amount: u64,
//     fee: u64,
//     sequence: u32,
//     last_ledger_sequence: Option<u32>,
//     destination_tag: Option<u32>,
//     rpc_url: Option<String>,
// ) -> Result<String, String> {
//     // First create/sign the transaction
//     let msg_id = create_or_sign_xrp_message(
//         account,
//         destination,
//         amount,
//         fee,
//         sequence,
//         last_ledger_sequence,
//         destination_tag,
//     ).await?;

//     // Check if transaction is fully signed
//     let is_signed = STATE.with(|state| {
//         state
//             .borrow()
//             .messages
//             .get(&msg_id)
//             .map(|m| m.signature.is_some())
//             .unwrap_or(false)
//     });

//     if is_signed {
//         // Submit to XRP ledger
//         submit_to_xrp_ledger(msg_id, rpc_url).await
//     } else {
//         Ok(format!("Transaction created but needs more signatures: {}", msg_id))
//     }
// }

// // Export candid interface
// ic_cdk::export_candid!();