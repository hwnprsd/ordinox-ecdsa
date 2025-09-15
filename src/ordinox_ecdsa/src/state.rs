use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(CandidType, Serialize, Deserialize, Debug, Default, Clone)]
pub struct ChainState {
    pub signers: Vec<Principal>,
    pub threshold: u32,
    pub network: String,
}

#[derive(CandidType, Serialize, Deserialize, Debug, Default, Clone)]
pub struct GlobalState {
    pub chains: HashMap<String, ChainState>,  // chain_id -> ChainState
}

thread_local! {
    pub static GLOBAL_STATE: RefCell<GlobalState> = RefCell::new(GlobalState::default());
}

pub fn init_chain(chain_id: String, signers: Vec<Principal>, threshold: u32, network: String) -> Result<(), String> {
    GLOBAL_STATE.with(|state| {
        let mut s = state.borrow_mut();
        s.chains.insert(chain_id, ChainState {
            signers,
            threshold,
            network,
        });
        Ok(())
    })
}

pub fn get_chain_config(chain_id: &str) -> Option<ChainState> {
    GLOBAL_STATE.with(|state| {
        state.borrow().chains.get(chain_id).cloned()
    })
}