use std::vec;

use ic_cdk:: update;
use alloy::{primitives::U256, signers::{icp::IcpSigner, Signature, Signer}};


pub struct EvmSignature {
    pub r: U256,
    pub s: U256,
    pub v: u64,  // In Ethereum, `v` is usually a single byte, but we’ll use `u64` and truncate it
}

impl EvmSignature {
    pub fn from_signature(s: Signature) -> EvmSignature {
        let recovery_id = s.v().to_u64();
        let v = recovery_id + 27;
        
        EvmSignature {
            r: s.r(),
            s: s.s(),
            v,
        }
    }

    pub fn to_hex_string(&self) -> String {
        // Convert r and s to 32-byte arrays
        let r_bytes: [u8; 32] = self.r.to_be_bytes();
        let s_bytes: [u8; 32] = self.s.to_be_bytes();
        
        // Convert v to a single byte (27 or 28 for Ethereum)
        let v_byte = self.v as u8;
        
        // Concatenate r || s || v
        let mut signature_bytes = Vec::with_capacity(65); // 32 + 32 + 1 = 65 bytes
        signature_bytes.extend_from_slice(&r_bytes);
        signature_bytes.extend_from_slice(&s_bytes);
        signature_bytes.push(v_byte);
        
        // Convert to hex string with "0x" prefix
        format!("0x{}", hex::encode(signature_bytes))}
}

fn get_ecdsa_key_name() -> String {
    #[allow(clippy::option_env_unwrap)]
    let dfx_network = option_env!("DFX_NETWORK").unwrap();
    match dfx_network {
        "local" => "dfx_test_key".to_string(),
        "ic" => "key_1".to_string(),
        _ => panic!("Unsupported network."),
    }
}

async fn create_icp_sepolia_signer() -> IcpSigner {
    let ecdsa_key_name = get_ecdsa_key_name();
    IcpSigner::new(vec![], &ecdsa_key_name, None).await.unwrap()
}

#[update]
async fn evm_pub_key() -> Result<String, String> {
    let signer = create_icp_sepolia_signer().await;
    Ok(hex::encode(signer.public_key()))
}


#[update]
async fn evm_address() -> Result<String, String> {
    super::query_pub_key().await.and_then(|pub_key| {
        Ok(ic_evm_utils::evm_signer::pubkey_bytes_to_address(&pub_key))
     })
}

pub(super) async fn sign_evm_message(msg: Vec<u8>) -> Result<String, String> {
    let signer = create_icp_sepolia_signer().await;
    match signer.sign_message(msg.as_slice()).await {
        Ok(signature) => Ok(EvmSignature::from_signature(signature).to_hex_string()), 
        Err(err) => Err(err.to_string())
    }
}

