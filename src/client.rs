mod protocol;

use protocol::{Message, Operation, StatusCode, generate_auth_token};
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const SERVER_PASSWORD: &str = "secure_password_123";

async fn send_message(stream: &mut TcpStream, message: &Message) -> Result<(), Box<dyn Error>> {
    let bytes = message.to_bytes();
    stream.write_all(&bytes).await?;
    Ok(())
}

async fn receive_message(stream: &mut TcpStream) -> Result<Message, Box<dyn Error>> {
    // Read length prefix
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes).await?;
    let length = u32::from_be_bytes(len_bytes);

    // Read message data
    let mut data = vec![0u8; length as usize];
    stream.read_exact(&mut data).await?;

    // Parse message
    Ok(Message::from_bytes(length, &data)?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    println!("Connected to server\n");

    let auth_token = generate_auth_token(SERVER_PASSWORD);
    let mut request_id = 1u32;

    // Step 1: Authenticate
    println!("=== Authenticating ===");
    let mut auth_msg = Message::new_with_auth(Operation::Auth, Vec::new(), auth_token);
    auth_msg.set_request_id(request_id);
    request_id += 1;

    send_message(&mut stream, &auth_msg).await?;
    let response = receive_message(&mut stream).await?;

    if response.status == StatusCode::Success {
        println!("✓ Authenticated successfully\n");
    } else {
        println!(
            "✗ Authentication failed: {}\n",
            String::from_utf8_lossy(&response.payload)
        );
        return Ok(());
    }

    // Step 2: Store a file
    println!("=== Test 1: STORE ===");
    let filename = "hello.txt";
    let file_content = b"Hello from the client! This is test data.";

    let mut payload = filename.as_bytes().to_vec();
    payload.push(0);
    payload.extend_from_slice(file_content);

    let mut store_msg = Message::new_with_auth(Operation::Store, payload, auth_token);
    store_msg.set_request_id(request_id);
    request_id += 1;

    send_message(&mut stream, &store_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!("Status: {:?}", response.status);
    println!("Response: {}\n", String::from_utf8_lossy(&response.payload));

    // Step 3: List files
    println!("=== Test 2: LIST ===");
    let mut list_msg = Message::new_with_auth(Operation::List, Vec::new(), auth_token);
    list_msg.set_request_id(request_id);
    request_id += 1;

    send_message(&mut stream, &list_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!(
        "Files on server:\n{}\n",
        String::from_utf8_lossy(&response.payload)
    );

    // Step 4: Retrieve the file
    println!("=== Test 3: RETRIEVE ===");
    let mut retrieve_msg = Message::new_with_auth(
        Operation::Retrieve,
        filename.as_bytes().to_vec(),
        auth_token,
    );
    retrieve_msg.set_request_id(request_id);
    request_id += 1;

    send_message(&mut stream, &retrieve_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!("Status: {:?}", response.status);
    println!(
        "Retrieved content: {}\n",
        String::from_utf8_lossy(&response.payload)
    );

    // Step 5: Try invalid auth (should fail)
    println!("=== Test 4: Invalid Auth Token (should fail) ===");
    let bad_token = [0u8; 32]; // Wrong token
    let mut bad_msg = Message::new_with_auth(Operation::List, Vec::new(), bad_token);
    bad_msg.set_request_id(request_id);
    request_id += 1;

    send_message(&mut stream, &bad_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!("Status: {:?}", response.status);
    println!("Response: {}\n", String::from_utf8_lossy(&response.payload));

    // Step 6: Delete a file
    println!("=== Test 5: DELETE ===");
    let mut delete_msg =
        Message::new_with_auth(Operation::Delete, filename.as_bytes().to_vec(), auth_token);
    delete_msg.set_request_id(request_id);
    request_id += 1;

    send_message(&mut stream, &delete_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!("Status: {:?}", response.status);
    println!("Response: {}\n", String::from_utf8_lossy(&response.payload));

    // Step 7: Try to retrieve deleted file
    println!("=== Test 6: RETRIEVE deleted file (should fail) ===");
    let mut retrieve_msg = Message::new_with_auth(
        Operation::Retrieve,
        filename.as_bytes().to_vec(),
        auth_token,
    );
    retrieve_msg.set_request_id(request_id);

    send_message(&mut stream, &retrieve_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!("Status: {:?}", response.status);
    println!("Response: {}\n", String::from_utf8_lossy(&response.payload));

    Ok(())
}
