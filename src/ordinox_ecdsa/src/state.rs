use alloy::primitives::{keccak256, Address, U256};
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use crate::sha256;

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub(super) struct EvmTransferMessage {
    pub signers: Vec<Principal>,
    pub to_address: String,
    pub token_address: String,
    pub amount: String,
    pub chain_id: String,
    pub signature: Option<String>,
    pub nonce: u64,
}

impl EvmTransferMessage {
    pub fn encode(self) -> String {
        let token_addr = Address::from_str(self.token_address.as_str()).unwrap();
        let to_addr = Address::from_str(self.to_address.as_str()).unwrap();
        let amt = U256::from_str(self.amount.as_str()).unwrap();
        let nonce = U256::from(self.nonce);
        let chain_id = U256::from_str(self.chain_id.as_str()).unwrap();

        let mut encoded: Vec<u8> = Vec::new();
        encoded.extend_from_slice(&nonce.as_le_bytes());
        encoded.extend_from_slice(&chain_id.as_le_bytes());
        encoded.extend_from_slice(token_addr.as_slice());
        encoded.extend_from_slice(to_addr.as_slice());
        encoded.extend_from_slice(&amt.as_le_bytes());

        keccak256(encoded).to_string()
    }

    pub fn hash(self) -> String {
        return self.encode();
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
