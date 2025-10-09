use candid::Principal;
use ic_cdk::api::management_canister::schnorr::{
    schnorr_public_key, sign_with_schnorr, SchnorrAlgorithm, SchnorrKeyId,
    SchnorrPublicKeyArgument, SignWithSchnorrArgument,
};
use ic_cdk_macros::{query, update};
use solana_sdk::{
    message::Message, pubkey::Pubkey, signature::Signature, signer::Signer as _,
    system_instruction, transaction::Transaction,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

thread_local! {
    static STATE: RefCell<SignerState> = RefCell::new(SignerState::default());
}

#[derive(Default)]
struct SignerState {
    signers: HashSet<Principal>,
    threshold: u8,
    approvals: HashMap<String, TransferRequest>,
}

#[derive(Clone)]
struct TransferRequest {
    nonce: u64,
    recipient: String,
    lamports: u64,
    signers: Vec<Principal>,
    signed: bool,
    signature: Option<String>,
}

fn schnorr_key_id() -> SchnorrKeyId {
    SchnorrKeyId {
        curve: SchnorrAlgorithm::Ed25519,
        name: "solana_key_1".to_string(),
    }
}

#[update]
async fn solana_address() -> String {
    let pubkey_resp = schnorr_public_key(SchnorrPublicKeyArgument {
        canister_id: None,
        derivation_path: vec![],
        key_id: schnorr_key_id(),
    })
    .await
    .unwrap();

    let pubkey = Pubkey::new(&pubkey_resp.public_key);
    pubkey.to_string()
}

#[update]
fn setup(signers: Vec<Principal>, threshold: u8) {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.signers = signers.into_iter().collect();
        st.threshold = threshold;
    });
}

#[update]
fn request_transfer(nonce: u64, recipient: String, lamports: u64) -> String {
    let caller = ic_cdk::caller();

    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if !st.signers.contains(&caller) {
            ic_cdk::trap("unauthorized");
        }
        let id = format!("{}:{}:{}", nonce, recipient, lamports);
        let entry = st.approvals.entry(id.clone()).or_insert(TransferRequest {
            nonce,
            recipient,
            lamports,
            signers: vec![],
            signed: false,
            signature: None,
        });
        if !entry.signers.contains(&caller) {
            entry.signers.push(caller);
        }
        id
    })
}

#[update]
async fn finalize_transfer(id: String) -> Result<String, String> {
    let key_id = schnorr_key_id();

    STATE
        .with(|s| async {
            let mut st = s.borrow_mut();
            let req = st.approvals.get_mut(&id).ok_or("request not found")?;

            if req.signed {
                return Ok(req.signature.clone().unwrap());
            }
            if req.signers.len() < st.threshold as usize {
                return Err("threshold not met".into());
            }

            let pubkey_resp = schnorr_public_key(SchnorrPublicKeyArgument {
                canister_id: None,
                derivation_path: vec![],
                key_id: key_id.clone(),
            })
            .await
            .map_err(|e| format!("failed to get pubkey: {:?}", e))?;

            let from_pubkey = Pubkey::new(&pubkey_resp.public_key);
            let to_pubkey =
                Pubkey::from_str(&req.recipient).map_err(|_| "invalid recipient pubkey")?;
            let instr = system_instruction::transfer(&from_pubkey, &to_pubkey, req.lamports);
            let msg = Message::new(&[instr], Some(&from_pubkey));
            let msg_bytes = msg.serialize();

            let sig_resp = sign_with_schnorr(SignWithSchnorrArgument {
                message: msg_bytes.clone(),
                derivation_path: vec![],
                key_id: key_id.clone(),
            })
            .await
            .map_err(|e| format!("sign failed: {:?}", e))?;

            req.signed = true;
            req.signature = Some(hex::encode(sig_resp.signature));

            Ok(req.signature.clone().unwrap())
        })
        .await
}

#[query]
fn get_signature(id: String) -> Option<String> {
    STATE.with(|s| {
        s.borrow()
            .approvals
            .get(&id)
            .and_then(|r| r.signature.clone())
    })
}
