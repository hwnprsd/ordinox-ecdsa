use candid::{CandidType, Decode, Encode, Principal};
use ic_agent::Agent;
use serde::Deserialize;

#[derive(CandidType, Deserialize, Debug)]
struct Message {
    id: u64,
    data: String,
    signers: Vec<Principal>,
    signature: Option<String>,
}

async fn setup_canister(agent: &Agent, canister_id: Principal) -> Result<(), String> {
    let signers = vec![Principal::from_text("2vxsx-fae").unwrap()];
    let threshold = 1u32;

    let args = Encode!(&signers, &threshold).unwrap();
    let response = agent
        .update(&canister_id, "setup")
        .with_arg(args)
        .call_and_wait()
        .await
        .map_err(|e| format!("Error calling setup: {:?}", e))?;

    let result: Result<(), String> = Decode!(response.as_slice(), Result<(), String>)
        .map_err(|e| format!("Error decoding setup response: {:?}", e))?;

    result
}

async fn create_or_sign_message(
    agent: &Agent,
    canister_id: Principal,
    message: String,
) -> Result<u64, String> {
    let args = Encode!(&message).unwrap();
    let response = agent
        .update(&canister_id, "create_or_sign_message")
        .with_arg(args)
        .call_and_wait()
        .await
        .map_err(|e| format!("Error calling create_or_sign_message: {:?}", e))?;

    let result: Result<u64, String> = Decode!(response.as_slice(), Result<u64, String>)
        .map_err(|e| format!("Error decoding create_or_sign_message response: {:?}", e))?;

    result
}

async fn get_message(
    agent: &Agent,
    canister_id: Principal,
    id: u64,
) -> Result<Option<Message>, String> {
    let args = Encode!(&id).unwrap();
    let response = agent
        .query(&canister_id, "get_message")
        .with_arg(args)
        .call()
        .await
        .map_err(|e| format!("Error calling get_message: {:?}", e))?;

    let result: Option<Message> = Decode!(response.as_slice(), Option<Message>)
        .map_err(|e| format!("Error decoding get_message response: {:?}", e))?;

    Ok(result)
}

#[tokio::test]
async fn test_threshold_ecdsa_canister() -> Result<(), String> {
    let url = "http://localhost:4943";
    let canister_id = Principal::from_text("br5f7-7uaaa-aaaaa-qaaca-cai").unwrap(); // Replace with your canister ID
    let agent = Agent::builder()
        .with_url(url)
        .build()
        .map_err(|e| format!("Error creating agent: {:?}", e))?;
    agent
        .fetch_root_key()
        .await
        .map_err(|e| format!("Error fetching root key: {:?}", e))?;

    // Setup the canister
    setup_canister(&agent, canister_id).await?;

    // Create a message
    let message_id =
        create_or_sign_message(&agent, canister_id, "Hello, World!".to_string()).await?;
    println!("Created message with ID: {}", message_id);

    // Sign the message (this will actually just add another signature)
    let _ = create_or_sign_message(&agent, canister_id, "Hello, World!".to_string()).await?;

    // Get the message
    let message = get_message(&agent, canister_id, message_id).await?;
    println!("Retrieved message: {:?}", message);

    // Check if the message was signed
    if let Some(msg) = message {
        assert_eq!(msg.data, "Hello, World!");
        assert_eq!(msg.signers.len(), 1);
        assert!(msg.signature.is_some(), "Message should be signed");
    } else {
        return Err("Message not found".to_string());
    }

    Ok(())
}
