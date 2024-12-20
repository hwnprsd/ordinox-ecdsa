use alloy::primitives::keccak256;
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::u128;

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub(super) struct EvmTransferMessage {
    pub signers: Vec<Principal>,
    pub to_address: String,
    pub token_address: String,
    pub amount: String,
    pub chain_id: u128,
    pub signature: Option<String>,
    pub nonce: u128,
}

impl EvmTransferMessage {
    pub fn encode_packed(self) -> Vec<u8> {
        let token_addr_bytes = hex::decode(&self.token_address.trim_start_matches("0x"))
            .expect("Invalid token address");
        let to_addr_bytes =
            hex::decode(&self.to_address.trim_start_matches("0x")).expect("Invalid to address");
        // Parse `amount` string into a 256-bit BigUint and convert it to a 32-byte array
        let amt = num_bigint::BigUint::from_str(&self.amount).expect("Invalid amount format");
        let mut amt_bytes = amt.to_bytes_be();
        // Pad amount to 32 bytes if it's less than that
        while amt_bytes.len() < 32 {
            amt_bytes.insert(0, 0); // Pad with leading zeros
        }

        let chain_id_bytes = self.chain_id.to_be_bytes();
        let nonce_bytes = self.nonce.to_be_bytes();

        let mut encoded: Vec<u8> = Vec::new();
        encoded.extend_from_slice(&nonce_bytes);
        encoded.extend_from_slice(&chain_id_bytes);
        encoded.extend_from_slice(&token_addr_bytes);
        encoded.extend_from_slice(&to_addr_bytes);
        encoded.extend_from_slice(&amt_bytes);

        keccak256(encoded).to_vec()
    }

    pub fn hash(self) -> String {
        return hex::encode(self.encode_packed());
    }
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub(super) struct Message {
    pub id: String,
    pub data: String,
    pub signers: Vec<Principal>,
    pub signature: Option<String>,
}

// State should have contract & rpc address
#[derive(CandidType, Serialize, Deserialize, Debug, Default, Clone)]
pub(super) struct State {
    pub signers: HashSet<Principal>,
    pub threshold: u32,
    pub messages: HashMap<String, EvmTransferMessage>,
}

thread_local! {
    pub(super) static STATE: RefCell<State> = RefCell::new(State::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_function_logging() {
        // Sample data for testing
        let signers = vec![Principal::from_text("aaaaa-aa").unwrap()];
        let to_address = "0x000000000000000000000000000000000000beef".to_string();
        let token_address = "0x000000000000000000000000000000000000dead".to_string();
        let amount = "1".to_string(); // 1 ETH in wei
        let chain_id = 1; // Ethereum Mainnet
        let nonce = 1;

        // Create an instance of EvmTransferMessage
        let message = EvmTransferMessage {
            signers,
            to_address,
            token_address,
            amount,
            chain_id,
            signature: None,
            nonce,
        };

        // Log the encoded output
        let encoded_output = message.hash();
        println!("Encoded Output: {}", encoded_output);
    }

    #[test]
    fn test_logic() {
        let mut x: u128 = 123;
        let _ans1 = take_u128(&mut x);
        let _ans2 = take_u128(&mut x);
        println!("{}", x)
    }

    fn take_u128(val: &mut u128) -> String {
        *val = *val + 1;
        return "OK".to_string();
    }
}
