mod protocol;

use protocol::{Message, Operation};
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server listening to 127.0.0.1:8080");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New connection from: {}", addr);

        tokio::spawn(async move {
            if let Err(e) = handle_client(socket).await {
                eprint!("Error handling client: {}", e);
            }
        });
    }
}

async fn handle_client(mut socket: TcpStream) -> Result<(), Box<dyn Error>> {
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
        println!("Receiving message of length: {}", length);

        // Read message data
        let mut data = vec![0u8; length as usize];
        socket.read_exact(&mut data).await?;

        // Parse message
        let message = Message::from_bytes(length, &data)?;
        println!("Received {:?} operation", message.operation);

        // Handle operation
        let response = handle_operation(message)?;

        // Send response
        socket.write_all(&response.to_bytes()).await?;
    }
}

fn handle_operation(message: Message) -> Result<Message, Box<dyn Error>> {
    match message.operation {
        Operation::Store => {
            let filename = String::from_utf8_lossy(&message.payload);
            println!("STORE request for: {}", filename);
            Ok(Message::new(Operation::Store, b"Ok: File stored".to_vec()))
        }

        Operation::Retrieve => {
            let filename = String::from_utf8_lossy(&message.payload);
            println!("RETRIEVE request for: {}", filename);
            Ok(Message::new(
                Operation::Retrieve,
                b"File contents here".to_vec(),
            ))
        }

        Operation::Delete => {
            let filename = String::from_utf8_lossy(&message.payload);
            println!("DELETE request for: {}", filename);
            Ok(Message::new(
                Operation::Delete,
                b"OK: File deleted".to_vec(),
            ))
        }

        Operation::List => {
            println!("LIST request");
            Ok(Message::new(
                Operation::List,
                b"file1.txt\nfile2.txt".to_vec(),
            ))
        }
    }
}
