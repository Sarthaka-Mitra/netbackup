use crate::protocol::{generate_auth_token, ChunkMetadata, Message, Operation, StatusCode};
use crate::storage::Storage;
use std::error::Error;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub async fn run(
    bind_addr: String,
    storage_path: String,
    password: String,
) -> Result<(), Box<dyn Error>> {
    let storage = Arc::new(Storage::new(&storage_path)?);
    println!("Storage initialized at: {}", storage_path);

    // CHANGE: Use password parameter instead of SERVER_PASSWORD
    let auth_token = generate_auth_token(&password);
    println!("Server auth token configured");

    let listener = TcpListener::bind(&bind_addr).await?;
    println!("Server listening on {}", bind_addr);
    println!("Access from other devices using your local IP address\n");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("[{}] New connection", addr);

        let storage = Arc::clone(&storage);
        tokio::spawn(async move {
            if let Err(e) = handle_client(socket, storage, auth_token).await {
                eprintln!("[{}] Error:  {}", addr, e);
            }
        });
    }
}

async fn handle_client(
    mut socket: TcpStream,
    storage: Arc<Storage>,
    expected_token: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    let peer_addr = socket.peer_addr()?;
    let mut authenticated = false;
    let mut request_counter = 0u32;

    loop {
        let mut len_bytes = [0u8; 4];
        match socket.read_exact(&mut len_bytes).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                println!("[{}] Client disconnected", peer_addr);
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        }

        let length = u32::from_be_bytes(len_bytes);
        let mut data = vec![0u8; length as usize];
        socket.read_exact(&mut data).await?;

        let message = match Message::from_bytes(length, &data) {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("[{}] Failed to parse message: {}", peer_addr, e);
                let error_response = Message::new_response(
                    request_counter,
                    Operation::Store,
                    StatusCode::ErrorInvalidData,
                    b"Invalid message format".to_vec(),
                );
                socket.write_all(&error_response.to_bytes()).await?;
                continue;
            }
        };

        request_counter += 1;

        if !matches!(message.operation, Operation::Auth) {
            if !authenticated {
                let response = Message::new_response(
                    message.request_id,
                    message.operation,
                    StatusCode::ErrorPermissionDenied,
                    b"Authentication required".to_vec(),
                );
                socket.write_all(&response.to_bytes()).await?;
                continue;
            }

            if message.auth_token != expected_token {
                let response = Message::new_response(
                    message.request_id,
                    message.operation,
                    StatusCode::ErrorPermissionDenied,
                    b"Invalid authentication token".to_vec(),
                );
                socket.write_all(&response.to_bytes()).await?;
                continue;
            }
        }

        let response = if message.operation == Operation::Auth {
            if message.auth_token == expected_token {
                authenticated = true;
                println!("[{}] ✓ Client authenticated", peer_addr);
                Message::new_response(
                    message.request_id,
                    Operation::Auth,
                    StatusCode::Success,
                    b"Authenticated".to_vec(),
                )
            } else {
                println!("[{}] ✗ Authentication failed", peer_addr);
                Message::new_response(
                    message.request_id,
                    Operation::Auth,
                    StatusCode::ErrorPermissionDenied,
                    b"Invalid password".to_vec(),
                )
            }
        } else {
            handle_storage_operation(message, &storage)
        };

        socket.write_all(&response.to_bytes()).await?;
    }
}

fn handle_storage_operation(message: Message, storage: &Storage) -> Message {
    match message.operation {
        Operation::StoreChunk => match ChunkMetadata::from_payload(&message.payload) {
            Ok(chunk) => {
                match storage.store_chunk(
                    &chunk.filename,
                    chunk.chunk_number,
                    chunk.total_chunks,
                    chunk.data,
                ) {
                    Ok(complete) => {
                        if complete {
                            println!(
                                "✓ CHUNK: {} - {}/{} (COMPLETE)",
                                chunk.filename,
                                chunk.chunk_number + 1,
                                chunk.total_chunks
                            );
                        } else {
                            println!(
                                "✓ CHUNK: {} - {}/{}",
                                chunk.filename,
                                chunk.chunk_number + 1,
                                chunk.total_chunks
                            );
                        }

                        let status = if complete { "COMPLETE" } else { "OK" };
                        Message::new_response(
                            message.request_id,
                            Operation::StoreChunk,
                            StatusCode::Success,
                            status.as_bytes().to_vec(),
                        )
                    }
                    Err(_) => {
                        eprintln!("✗ CHUNK STORE failed");
                        Message::new_response(
                            message.request_id,
                            Operation::StoreChunk,
                            StatusCode::ErrorServerError,
                            b"Chunk storage failed".to_vec(),
                        )
                    }
                }
            }
            Err(_) => Message::new_response(
                message.request_id,
                Operation::StoreChunk,
                StatusCode::ErrorInvalidData,
                b"Invalid chunk metadata".to_vec(),
            ),
        },
        Operation::StoreComplete => {
            let filename = String::from_utf8_lossy(&message.payload).to_string();

            match storage.complete_chunked_upload(&filename) {
                Ok(_) => {
                    println!("✓ STORE COMPLETE: {}", filename);
                    Message::new_response(
                        message.request_id,
                        Operation::StoreComplete,
                        StatusCode::Success,
                        b"File stored successfully".to_vec(),
                    )
                }
                Err(_) => {
                    eprintln!("✗ STORE COMPLETE failed");
                    Message::new_response(
                        message.request_id,
                        Operation::StoreComplete,
                        StatusCode::ErrorServerError,
                        b"Failed to finalize upload".to_vec(),
                    )
                }
            }
        }
        Operation::Store => {
            let null_pos = match message.payload.iter().position(|&b| b == 0) {
                Some(pos) => pos,
                None => {
                    return Message::new_response(
                        message.request_id,
                        Operation::Store,
                        StatusCode::ErrorInvalidData,
                        b"Invalid STORE payload".to_vec(),
                    );
                }
            };

            let filename = String::from_utf8_lossy(&message.payload[..null_pos]).to_string();
            let file_data = &message.payload[null_pos + 1..];

            match storage.store(&filename, file_data) {
                Ok(_) => {
                    println!("✓ STORE: {} ({} bytes)", filename, file_data.len());
                    Message::new_response(
                        message.request_id,
                        Operation::Store,
                        StatusCode::Success,
                        b"OK".to_vec(),
                    )
                }
                Err(_) => {
                    eprintln!("✗ STORE failed");
                    Message::new_response(
                        message.request_id,
                        Operation::Store,
                        StatusCode::ErrorServerError,
                        b"Storage failed".to_vec(),
                    )
                }
            }
        }
        Operation::Retrieve => {
            let filename = String::from_utf8_lossy(&message.payload).to_string();

            match storage.retrieve(&filename) {
                Ok(data) => {
                    println!("✓ RETRIEVE: {} ({} bytes)", filename, data.len());
                    Message::new_response(
                        message.request_id,
                        Operation::Retrieve,
                        StatusCode::Success,
                        data,
                    )
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    eprintln!("✗ RETRIEVE failed: File not found");
                    Message::new_response(
                        message.request_id,
                        Operation::Retrieve,
                        StatusCode::ErrorNotFound,
                        b"File not found".to_vec(),
                    )
                }
                Err(_) => {
                    eprintln!("✗ RETRIEVE failed");
                    Message::new_response(
                        message.request_id,
                        Operation::Retrieve,
                        StatusCode::ErrorServerError,
                        b"Retrieval failed".to_vec(),
                    )
                }
            }
        }
        Operation::Delete => {
            let filename = String::from_utf8_lossy(&message.payload).to_string();

            match storage.delete(&filename) {
                Ok(_) => {
                    println!("✓ DELETE: {}", filename);
                    Message::new_response(
                        message.request_id,
                        Operation::Delete,
                        StatusCode::Success,
                        b"OK".to_vec(),
                    )
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    eprintln!("✗ DELETE failed: File not found");
                    Message::new_response(
                        message.request_id,
                        Operation::Delete,
                        StatusCode::ErrorNotFound,
                        b"File not found".to_vec(),
                    )
                }
                Err(_) => {
                    eprintln!("✗ DELETE failed");
                    Message::new_response(
                        message.request_id,
                        Operation::Delete,
                        StatusCode::ErrorServerError,
                        b"Deletion failed".to_vec(),
                    )
                }
            }
        }
        Operation::List => match storage.list() {
            Ok(files) => {
                let payload = bincode::serialize(&files).unwrap(); // Or serde_json
                println!("✓ LIST: {} files", files.len());
                Message::new_response(
                    message.request_id,
                    Operation::List,
                    StatusCode::Success,
                    payload,
                )
            }
            Err(_) => {
                eprintln!("✗ LIST failed");
                Message::new_response(
                    message.request_id,
                    Operation::List,
                    StatusCode::ErrorServerError,
                    b"List failed".to_vec(),
                )
            }
        },
        Operation::Auth => Message::new_response(
            message.request_id,
            Operation::Auth,
            StatusCode::ErrorServerError,
            b"Unexpected auth operation".to_vec(),
        ),
        Operation::RetrieveChunk => Message::new_response(
            message.request_id,
            Operation::RetrieveChunk,
            StatusCode::ErrorServerError,
            b"Chunked retrieval not yet implemented".to_vec(),
        ),
    }
}
