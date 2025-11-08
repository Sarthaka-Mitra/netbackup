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
    let mut buffer = vec![0u8; 4096];

    loop {
        let n = socket.read(&mut buffer).await?;

        //Connection closed
        if n == 0 {
            println!("Connection terminated");
            return Ok(());
        }

        println!("Received {} bytes", n);

        //Echo back
        socket.write_all(&buffer[..n]).await?;
    }
}
