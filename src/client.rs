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

    // Test 1: Store a file
    println!("=== Test 1: STORE ===");
    let filename = "hello.txt";
    let file_content = b"Hello from the client! This is test data.";

    // Create payload: filename + null byte + data
    let mut payload = filename.as_bytes().to_vec();
    payload.push(0);
    payload.extend_from_slice(file_content);

    let store_msg = Message::new(Operation::Store, payload);
    send_message(&mut stream, &store_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!("Response: {}\n", String::from_utf8_lossy(&response.payload));

    // Test 2: List files
    println!("=== Test 2: LIST ===");
    let list_msg = Message::new(Operation::List, Vec::new());
    send_message(&mut stream, &list_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!(
        "Files on server:\n{}\n",
        String::from_utf8_lossy(&response.payload)
    );

    // Test 3: Retrieve the file
    println!("=== Test 3: RETRIEVE ===");
    let retrieve_msg = Message::new(Operation::Retrieve, filename.as_bytes().to_vec());
    send_message(&mut stream, &retrieve_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!(
        "Retrieved content: {}\n",
        String::from_utf8_lossy(&response.payload)
    );

    // Test 4: Store another file
    println!("=== Test 4: STORE another file ===");
    let filename2 = "data.txt";
    let content2 = b"More test data here! touching the file so that i can see i have edited the text and stuff";

    let mut payload2 = filename2.as_bytes().to_vec();
    payload2.push(0);
    payload2.extend_from_slice(content2);

    let store_msg2 = Message::new(Operation::Store, payload2);
    send_message(&mut stream, &store_msg2).await?;
    let response = receive_message(&mut stream).await?;
    println!("Response: {}\n", String::from_utf8_lossy(&response.payload));

    // Test 5: List again
    println!("=== Test 5: LIST (should show 2 files) ===");
    let list_msg = Message::new(Operation::List, Vec::new());
    send_message(&mut stream, &list_msg).await?;
    let response = receive_message(&mut stream).await?;
    println!(
        "Files on server:\n{}\n",
        String::from_utf8_lossy(&response.payload)
    );

    // Test 6: Delete a file
    // println!("=== Test 6: DELETE ===");
    // let delete_msg = Message::new(Operation::Delete, filename.as_bytes().to_vec());
    // send_message(&mut stream, &delete_msg).await?;
    // let response = receive_message(&mut stream).await?;
    // println!("Response: {}\n", String::from_utf8_lossy(&response.payload));
    //
    // // Test 7: List after delete
    // println!("=== Test 7: LIST (should show 1 file) ===");
    // let list_msg = Message::new(Operation::List, Vec::new());
    // send_message(&mut stream, &list_msg).await?;
    // let response = receive_message(&mut stream).await?;
    // println!(
    //     "Files on server:\n{}\n",
    //     String::from_utf8_lossy(&response.payload)
    // );
    //
    // // Test 8: Try to retrieve deleted file (should fail)
    // println!("=== Test 8: RETRIEVE deleted file (should error) ===");
    // let retrieve_msg = Message::new(Operation::Retrieve, filename.as_bytes().to_vec());
    // send_message(&mut stream, &retrieve_msg).await?;
    // let response = receive_message(&mut stream).await?;
    // println!("Response: {}\n", String::from_utf8_lossy(&response.payload));

    Ok(())
}
