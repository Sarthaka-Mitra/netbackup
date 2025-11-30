mod protocol;
mod storage;

use protocol::{Message, Operation};
use std::error::Error;
use std::sync::Arc;
use storage::Storage;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //Initialise storage
    let storage = Arc::new(Storage::new("./storage_data")?);
    println!("Storage initialized at: ./storage_data");

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server listening to 127.0.0.1:8080");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New connection from: {}", addr);

        let storage = Arc::clone(&storage);
        tokio::spawn(async move {
            if let Err(e) = handle_client(socket, storage).await {
                eprintln!("Error handling client: {}", e);
            }
        });
    }
}

async fn handle_client(mut socket: TcpStream, storage: Arc<Storage>) -> Result<(), Box<dyn Error>> {
    loop {
        // Read length prefix (4 bytes)
        let mut len_bytes = [0u8; 4];
        match socket.read_exact(&mut len_bytes).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                println!("Client disconnected");
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        }

        let length = u32::from_be_bytes(len_bytes);

        // Read message data
        let mut data = vec![0u8; length as usize];
        socket.read_exact(&mut data).await?;

        // Parse message
        let message = Message::from_bytes(length, &data)?;

        // Handle operation
        let response = handle_operation(message, &storage)?;

        // Send response
        socket.write_all(&response.to_bytes()).await?;
    }
}

fn handle_operation(message: Message, storage: &Storage) -> Result<Message, Box<dyn Error>> {
    match message.operation {
        Operation::Store => {
            // Payload format: filename + null byte + file data
            let null_pos = message
                .payload
                .iter()
                .position(|&b| b == 0)
                .ok_or("Invalid STORE payload: missing null separator")?;

            let filename = String::from_utf8_lossy(&message.payload[..null_pos]).to_string();
            let file_data = &message.payload[null_pos + 1..];

            match storage.store(&filename, file_data) {
                Ok(_) => {
                    println!("✓ STORE: {} ({} bytes)", filename, file_data.len());
                    Ok(Message::new(Operation::Store, b"OK".to_vec()))
                }
                Err(e) => {
                    eprintln!("✗ STORE failed: {}", e);
                    Ok(Message::new(
                        Operation::Store,
                        format!("ERROR: {}", e).into_bytes(),
                    ))
                }
            }
        }

        Operation::Retrieve => {
            let filename = String::from_utf8_lossy(&message.payload).to_string();

            match storage.retrieve(&filename) {
                Ok(data) => {
                    println!("RETRIEVE: {} ({} bytes)", filename, data.len());
                    Ok(Message::new(Operation::Retrieve, data))
                }
                Err(e) => {
                    eprintln!("RETRIEVE failed: {}", e);
                    Ok(Message::new(
                        Operation::Retrieve,
                        format!("ERROR: {}", e).into_bytes(),
                    ))
                }
            }
        }

        Operation::Delete => {
            let filename = String::from_utf8_lossy(&message.payload).to_string();

            match storage.delete(&filename) {
                Ok(_) => {
                    println!("✓ DELETE: {}", filename);
                    Ok(Message::new(Operation::Delete, b"OK".to_vec()))
                }
                Err(e) => {
                    eprintln!("DELETE failed: {}", e);
                    Ok(Message::new(
                        Operation::Delete,
                        format!("ERROR: {}", e).into_bytes(),
                    ))
                }
            }
        }

        Operation::List => match storage.list() {
            Ok(files) => {
                let file_list = files.join("\n");
                println!("✓ LIST: {} files", files.len());
                Ok(Message::new(Operation::List, file_list.into_bytes()))
            }
            Err(e) => {
                eprintln!("✗ LIST failed: {}", e);
                Ok(Message::new(
                    Operation::List,
                    format!("ERROR: {}", e).into_bytes(),
                ))
            }
        },
    }
}
