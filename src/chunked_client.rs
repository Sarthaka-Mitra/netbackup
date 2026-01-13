mod protocol;

use protocol::{CHUNK_SIZE, ChunkMetadata, Message, Operation, StatusCode, generate_auth_token};
use std::error::Error;
use std::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const SERVER_PASSWORD: &str = "secure_password_123";

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

async fn upload_file_chunked(
    stream: &mut TcpStream,
    filename: &str,
    data: &[u8],
    auth_token: [u8; 32],
    request_id: &mut u32,
) -> Result<(), Box<dyn Error>> {
    let total_size = data.len();
    let total_chunks = (total_size + CHUNK_SIZE - 1) / CHUNK_SIZE;

    println!(
        "Uploading {} ({} bytes) in {} chunks...",
        filename, total_size, total_chunks
    );

    for chunk_num in 0..total_chunks {
        let start = chunk_num * CHUNK_SIZE;
        let end = std::cmp::min(start + CHUNK_SIZE, total_size);
        let chunk_data = data[start..end].to_vec();

        let chunk_meta = ChunkMetadata {
            filename: filename.to_string(),
            chunk_number: chunk_num as u32,
            total_chunks: total_chunks as u32,
            data: chunk_data,
        };

        let mut msg =
            Message::new_with_auth(Operation::StoreChunk, chunk_meta.to_payload(), auth_token);
        msg.set_request_id(*request_id);
        *request_id += 1;

        send_message(stream, &msg).await?;
        let response = receive_message(stream).await?;

        if response.status != StatusCode::Success {
            return Err(format!(
                "Chunk {} failed: {}",
                chunk_num,
                String::from_utf8_lossy(&response.payload)
            )
            .into());
        }

        print!("\rProgress: {}/{} chunks", chunk_num + 1, total_chunks);
        std::io::Write::flush(&mut std::io::stdout())?;
    }

    println!("\n✓ All chunks sent");

    // Signal completion
    let mut complete_msg = Message::new_with_auth(
        Operation::StoreComplete,
        filename.as_bytes().to_vec(),
        auth_token,
    );
    complete_msg.set_request_id(*request_id);
    *request_id += 1;

    send_message(stream, &complete_msg).await?;
    let response = receive_message(stream).await?;

    if response.status == StatusCode::Success {
        println!("✓ Upload complete!");
        Ok(())
    } else {
        Err(format!(
            "Upload finalization failed: {}",
            String::from_utf8_lossy(&response.payload)
        )
        .into())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    println!("Connected to server\n");

    let auth_token = generate_auth_token(SERVER_PASSWORD);
    let mut request_id = 1u32;

    // Authenticate
    println!("=== Authenticating ===");
    let mut auth_msg = Message::new_with_auth(Operation::Auth, Vec::new(), auth_token);
    auth_msg.set_request_id(request_id);
    request_id += 1;

    send_message(&mut stream, &auth_msg).await?;
    let response = receive_message(&mut stream).await?;

    if response.status != StatusCode::Success {
        println!("✗ Authentication failed");
        return Ok(());
    }
    println!("✓ Authenticated\n");

    // Test 1: Upload a small file (single chunk)
    println!("=== Test 1: Small File (1 chunk) ===");
    let small_data = b"This is a small file that fits in one chunk.".to_vec();
    upload_file_chunked(
        &mut stream,
        "small.txt",
        &small_data,
        auth_token,
        &mut request_id,
    )
    .await?;
    println!();

    // Test 2: Upload a larger file (multiple chunks)
    println!("=== Test 2: Large File (multiple chunks) ===");
    let large_data = vec![b'X'; 200_000]; // 200KB file
    upload_file_chunked(
        &mut stream,
        "large.txt",
        &large_data,
        auth_token,
        &mut request_id,
    )
    .await?;
    println!();

    // Test 3: Upload a file from disk (if available)
    println!("=== Test 3: Real File Upload ===");
    match fs::read("Cargo.toml") {
        Ok(file_data) => {
            upload_file_chunked(
                &mut stream,
                "Cargo.toml",
                &file_data,
                auth_token,
                &mut request_id,
            )
            .await?;
        }
        Err(_) => {
            println!("Skipping (Cargo.toml not found)");
        }
    }
    println!();

    // List files
    println!("=== Listing Files ===");
    let mut list_msg = Message::new_with_auth(Operation::List, Vec::new(), auth_token);
    list_msg.set_request_id(request_id);
    request_id += 1;

    send_message(&mut stream, &list_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!(
        "Files on server:\n{}\n",
        String::from_utf8_lossy(&response.payload)
    );

    // Verify retrieval
    println!("=== Verifying Upload ===");
    let mut retrieve_msg =
        Message::new_with_auth(Operation::Retrieve, b"small.txt".to_vec(), auth_token);
    retrieve_msg.set_request_id(request_id);

    send_message(&mut stream, &retrieve_msg).await?;
    let response = receive_message(&mut stream).await?;

    if response.payload == small_data {
        println!("✓ File integrity verified!");
    } else {
        println!("✗ File data mismatch!");
    }

    Ok(())
}
