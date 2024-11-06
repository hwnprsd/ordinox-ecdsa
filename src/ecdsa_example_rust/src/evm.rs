use ic_cdk::{query, update};
use alloy::{
    network::{NetworkWallet, TxSigner}, signers::icp::IcpSigner, transports::icp::{RpcApi, RpcService}
};
use alloy::{
    network::EthereumWallet,
    primitives::{address, U256, Signature, SignatureError},
    providers::{Provider, ProviderBuilder},
    signers::Signer,
    sol,
    transports::icp::IcpConfig,
};

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

async fn sign_evm_message(msg: &[u8]) -> Result<String, String> {
    let signer = create_icp_sepolia_signer().await;
    match signer.sign_message(msg).await {
        Ok(signature) => signature.to_k256().and_then(|s| Ok(s.to_string())).map_err(|err| err.to_string()), 
        Err(err) => Err(err.to_string())
    }
}



