mod protocol;

use protocol::{Message, Operation};
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

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
    println!("Connected to server");

    // Test STORE operation
    println!("=== Testing STORE ===");
    let store_msg = Message::new(Operation::Store, b"myfile.txt".to_vec());
    send_message(&mut stream, &store_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!("Response: {}\n", String::from_utf8_lossy(&response.payload));

    // Test RETRIEVE operation
    println!("=== Testing RETRIEVE ===");
    let retrieve_msg = Message::new(Operation::Retrieve, b"myfile.txt".to_vec());
    send_message(&mut stream, &retrieve_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!("Response: {}\n", String::from_utf8_lossy(&response.payload));

    // Test LIST operation
    println!("=== Testing LIST ===");
    let list_msg = Message::new(Operation::List, Vec::new());
    send_message(&mut stream, &list_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!(
        "Response:\n{}\n",
        String::from_utf8_lossy(&response.payload)
    );

    //Test DELETE operation
    //println!("=== Testing DELETE ===");
    //let delete_msg = Message::new(Operation::Delete, b"myfile.txt".to_vec());
    //send_message(&mut stream, &delete_msg).await?;
    //let response = receive_message(&mut stream).await?;
    //println!("Response: {}\n", String::from_utf8_lossy(&response.payload));
    Ok(())
}
