use candid::{CandidType, Principal};
use ic_cdk::api::call;
use ic_cdk::api::management_canister::ecdsa::{
    sign_with_ecdsa, EcdsaCurve, EcdsaKeyId, SignWithEcdsaArgument,
};
use ic_cdk::{query, update};
use serde::{Deserialize, Serialize};
use std::cell::{RefCell, RefMut};
use std::collections::HashSet;

#[query]
fn state() -> State {
    STATE.with(|state| state.borrow().clone())
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
struct Message {
    id: u64,
    data: String,
    signers: Vec<Principal>,
    signature: Option<String>,
}

#[derive(CandidType, Serialize, Deserialize, Debug, Default, Clone)]
struct State {
    signers: HashSet<Principal>,
    threshold: u32,
    messages: Vec<Message>,
    next_id: u64,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[update]
fn setup(signers: Vec<Principal>, threshold: u32) -> String {
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        if !s.signers.is_empty() {
            // This is STRICTLY ONLY FOR TESTING
            // THIS SHOULD THROW AN ERR IN PRODUCTION
            // INSTEAD, RESET THE STATE FOR TESTING
            s.messages = Vec::new();
            s.signers = HashSet::new();
            s.next_id = 0;
            s.threshold = 0;
        }
        if threshold as usize > signers.len() {
            return "Threshold cannot be greater than the number of signers".to_string();
        }
        s.signers = signers.into_iter().collect();
        s.threshold = threshold;
        "OK".to_string()
    })
}

#[query]
async fn caller() -> Principal {
    ic_cdk::caller()
}

#[update]
async fn create_or_sign_message(message: String) -> Result<u64, String> {
    let caller = ic_cdk::caller();

    // Check if the caller is an authorized signer and if the message exists
    let (is_authorized, message_exists, threshold) = STATE.with(|state| {
        let s = state.borrow();
        let is_authorized = s.signers.contains(&caller);
        let message_exists = s.messages.iter().any(|m| m.data == message);
        (is_authorized, message_exists, s.threshold)
    });

    if !is_authorized {
        return Err("Caller is not an authorized signer".to_string());
    }

    if message_exists {
        // Message exists, add signer
        let (id, should_sign) = STATE.with(|state| {
            let mut s = state.borrow_mut();
            let message = s.messages.iter_mut().find(|m| m.data == message).unwrap();
            if !message.signers.contains(&caller) {
                message.signers.push(caller);
            }
            (
                message.id,
                message.signers.len() as u32 >= threshold && message.signature.is_none(),
            )
        });

        if should_sign {
            // Threshold reached, trigger signing
            sign_message(id).await
        } else {
            Ok(id)
        }
    } else {
        // Create a new message
        STATE.with(|state| {
            let mut s = state.borrow_mut();
            let id = s.next_id;
            s.next_id += 1;
            let new_message = Message {
                id,
                data: message,
                signers: vec![caller],
                signature: None,
            };
            s.messages.push(new_message);
            Ok(id)
        })
    }
}

async fn sign_message(id: u64) -> Result<u64, String> {
    let message = STATE
        .with(|state| state.borrow().messages.iter().find(|m| m.id == id).cloned())
        .ok_or("Message not found")?;

    let request = SignWithEcdsaArgument {
        message_hash: sha256(&message.data).to_vec(),
        derivation_path: vec![],
        key_id: EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: "dfx_test_key".to_string(),
        },
    };

    match sign_with_ecdsa(request).await {
        Ok((response,)) => {
            let signature = hex::encode(response.signature);
            STATE.with(|state| {
                let mut s = state.borrow_mut();
                if let Some(msg) = s.messages.iter_mut().find(|m| m.id == id) {
                    msg.signature = Some(signature);
                }
            });
            Ok(id)
        }
        Err(e) => Err(format!("Failed to sign message: {:?}", e)),
    }
}

#[query]
fn get_signature(msg: String) -> String {
    STATE.with(|state| {
        state
            .borrow()
            .messages
            .iter()
            .find(|m| m.data == msg)
            .and_then(|m| m.signature.clone())
            .unwrap_or_else(|| "Message or signature not found".to_string())
    })
}

fn sha256(input: &str) -> [u8; 32] {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().into()
}

// Custom getrandom implementation
getrandom::register_custom_getrandom!(always_fail);
pub fn always_fail(_buf: &mut [u8]) -> Result<(), getrandom::Error> {
    Err(getrandom::Error::UNSUPPORTED)
}
