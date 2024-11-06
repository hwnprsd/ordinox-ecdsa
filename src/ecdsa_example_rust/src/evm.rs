use ic_cdk:: update;
use alloy::signers::{icp::IcpSigner, Signer};

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
async fn evm_address() -> Result<String, String> {
    super::query_pub_key().await.and_then(|pub_key| {
        Ok(ic_evm_utils::evm_signer::pubkey_bytes_to_address(&pub_key))
     })
}

pub(super) async fn sign_evm_message(msg: String) -> Result<String, String> {
    let signer = create_icp_sepolia_signer().await;
    match signer.sign_message(msg.as_bytes()).await {
        Ok(signature) => Ok(format!("{:?}", signature)), 
        Err(err) => Err(err.to_string())
    }
    // Ok("OK".to_string())
}



