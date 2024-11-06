use candid::Principal;
use ic_cdk::api::management_canister::ecdsa::{ ecdsa_public_key, EcdsaCurve, EcdsaKeyId,  EcdsaPublicKeyArgument };
use ic_cdk::{query, update};
use state::{STATE, State, EvmTransferMessage};

mod evm;
mod state;

#[query]
fn state() -> State {
    STATE.with(|state| state.borrow().clone())
}


#[update]
fn setup(signers: Vec<Principal>, threshold: u32) -> String {
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        if !s.signers.is_empty() {
            // This is STRICTLY ONLY FOR TESTING
            // THIS SHOULD THROW AN ERR IN PRODUCTION
            // INSTEAD, RESET THE STATE FOR TESTING
            s.messages.clear();
            s.signers.clear();
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
async fn create_or_sign_evm_message(nonce: u64, chain_id: String, token_address: String, to_address: String, amount: String) -> Result<String, String> {

    let mut msg = EvmTransferMessage {
        signers: vec![], 
        signature: None, 
        token_address, to_address, chain_id, amount, nonce,
    };

    let caller = ic_cdk::caller();
    let msg_id = msg.clone().hash();

    // Check if the caller is an authorized signer and if the message exists
    let (is_authorized, message_exists, threshold) = STATE.with(|state| {
        let s = state.borrow();
        let is_authorized = s.signers.contains(&caller);
        let message_exists = s.messages.contains_key(&msg_id);
        (is_authorized, message_exists, s.threshold)
    });

    if !is_authorized {
        return Err("Caller is not an authorized signer".to_string());
    }

    if message_exists {
        // Message exists, add signer
        let should_sign = STATE.with(|state| {
            let mut s = state.borrow_mut();
            let message = s.messages.get_mut(&msg_id).unwrap();
            if !message.signers.contains(&caller) {
                message.signers.push(caller);
            }
            message.signers.len() as u32 >= threshold && message.signature.is_none()
        });

        if should_sign {
            // Threshold reached, trigger signing
            sign_message(msg_id.clone()).await
        } else {
            Ok(msg_id)
        }
    } else {
        // Create a new message
        STATE.with(|state| {
            let mut s = state.borrow_mut();
            msg.signers.push(caller);
            s.messages.insert(msg_id.clone(), msg);
            Ok(msg_id)
        })
    }
}

async fn sign_message(id: String) -> Result<String, String> {
    // instead of just signing, call the contract
    let message = STATE
        .with(|state| state.borrow().messages.get(&id).cloned())
        .ok_or("Message not found")?;

    evm::sign_evm_message(message.hash()).await
}

#[query]
fn get_signature(msg_id: String) -> String {
    STATE.with(|state| {
        state
            .borrow()
            .messages
            .get(&msg_id)
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

async fn query_pub_key() -> Result<Vec<u8>, String> {
    let request = EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path: vec![],
        key_id: EcdsaKeyIds::TestKeyLocalDevelopment.to_key_id(),
    };

    let (response,) = ecdsa_public_key(request)
        .await
        .map_err(|e| format!("ecdsa_public_key failed {}", e.1))?;
    Ok(response.public_key)
}
 
#[update]
async fn public_key() -> Result<String, String> {
    query_pub_key().await.and_then(|pub_key| {
        Ok(hex::encode(pub_key))
    })
}


enum EcdsaKeyIds {
    #[allow(unused)]
    TestKeyLocalDevelopment,
    #[allow(unused)]
    TestKey1,
    #[allow(unused)]
    ProductionKey1,
}

impl EcdsaKeyIds {
    fn to_key_id(&self) -> EcdsaKeyId {
        EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: match self {
                Self::TestKeyLocalDevelopment => "dfx_test_key",
                Self::TestKey1 => "test_key_1",
                Self::ProductionKey1 => "key_1",
            }
            .to_string(),
        }
    }
}

