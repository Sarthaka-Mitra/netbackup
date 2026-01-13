mod protocol;

use protocol::{CHUNK_SIZE, ChunkMetadata, Message, Operation, StatusCode, generate_auth_token};
use std::env;
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const SERVER_PASSWORD: &str = "secure_password_123";
const DEFAULT_SERVER: &str = "127.0.0.1:8080";

async fn send_message(stream: &mut TcpStream, message: &Message) -> Result<(), Box<dyn Error>> {
    let bytes = message.to_bytes();
    stream.write_all(&bytes).await?;
    Ok(())
}

async fn receive_message(stream: &mut TcpStream) -> Result<Message, Box<dyn Error>> {
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes).await?;
    let length = u32::from_be_bytes(len_bytes);

    let mut data = vec![0u8; length as usize];
    stream.read_exact(&mut data).await?;

    Ok(Message::from_bytes(length, &data)?)
}

async fn client_task(client_id: usize, server_addr: String) -> Result<(), Box<dyn Error>> {
    let mut stream = TcpStream::connect(&server_addr).await?;
    println!("[Client {}] Connected", client_id);

    let auth_token = generate_auth_token(SERVER_PASSWORD);
    let mut request_id = 1u32;

    // Authenticate
    let mut auth_msg = Message::new_with_auth(Operation::Auth, Vec::new(), auth_token);
    auth_msg.set_request_id(request_id);
    request_id += 1;

    send_message(&mut stream, &auth_msg).await?;
    let response = receive_message(&mut stream).await?;

    if response.status != StatusCode::Success {
        return Err(format!("Client {} auth failed", client_id).into());
    }
    println!("[Client {}] Authenticated", client_id);

    // Upload a file unique to this client
    let filename = format!("client_{}_file.txt", client_id);
    let data = format!("Data from client {}", client_id)
        .repeat(1000)
        .into_bytes();

    let total_size = data.len();
    let total_chunks = (total_size + CHUNK_SIZE - 1) / CHUNK_SIZE;

    println!(
        "[Client {}] Uploading {} ({} bytes, {} chunks)",
        client_id, filename, total_size, total_chunks
    );

    // Upload chunks
    for chunk_num in 0..total_chunks {
        let start = chunk_num * CHUNK_SIZE;
        let end = std::cmp::min(start + CHUNK_SIZE, total_size);
        let chunk_data = data[start..end].to_vec();

        let chunk_meta = ChunkMetadata {
            filename: filename.clone(),
            chunk_number: chunk_num as u32,
            total_chunks: total_chunks as u32,
            data: chunk_data,
        };

        let mut msg =
            Message::new_with_auth(Operation::StoreChunk, chunk_meta.to_payload(), auth_token);
        msg.set_request_id(request_id);
        request_id += 1;

        send_message(&mut stream, &msg).await?;
        let response = receive_message(&mut stream).await?;

        if response.status != StatusCode::Success {
            return Err(format!("Client {} chunk upload failed", client_id).into());
        }
    }

    // Complete upload
    let mut complete_msg = Message::new_with_auth(
        Operation::StoreComplete,
        filename.as_bytes().to_vec(),
        auth_token,
    );
    complete_msg.set_request_id(request_id);
    request_id += 1;

    send_message(&mut stream, &complete_msg).await?;
    let response = receive_message(&mut stream).await?;

    if response.status == StatusCode::Success {
        println!("[Client {}] ✓ Upload complete!", client_id);
    } else {
        return Err(format!("Client {} finalization failed", client_id).into());
    }

    // Retrieve and verify
    let mut retrieve_msg = Message::new_with_auth(
        Operation::Retrieve,
        filename.as_bytes().to_vec(),
        auth_token,
    );
    retrieve_msg.set_request_id(request_id);

    send_message(&mut stream, &retrieve_msg).await?;
    let response = receive_message(&mut stream).await?;

    if response.payload == data {
        println!("[Client {}] ✓ Verification passed!", client_id);
    } else {
        println!("[Client {}] ✗ Verification FAILED!", client_id);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let server_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_SERVER.to_string());
    let num_clients: usize = env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(3);

    println!(
        "Starting {} concurrent clients connecting to {}\n",
        num_clients, server_addr
    );

    let mut handles = vec![];

    for i in 0..num_clients {
        let addr = server_addr.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = client_task(i, addr).await {
                eprintln!("[Client {}] Error: {}", i, e);
            }
        });
        handles.push(handle);
    }

    // Wait for all clients to finish
    for handle in handles {
        handle.await?;
    }

    println!("\n✓ All clients finished!");

    Ok(())
}
