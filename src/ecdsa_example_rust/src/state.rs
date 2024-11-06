use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

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
        format!(
            "{}|{}|{}|{}|{}",
            self.nonce, self.chain_id, self.token_address, self.to_address, self.amount
        )
    }

    pub fn hash(self) -> String {
        hex::encode(sha256(&self.encode()))
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
