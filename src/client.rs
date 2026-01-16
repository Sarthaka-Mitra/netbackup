use crate::protocol::{
    generate_auth_token, ChunkMetadata, Message, Operation, StatusCode, CHUNK_SIZE,
};
use std::error::Error;
use std::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

struct Client {
    stream: TcpStream,
    auth_token: [u8; 32],
    request_id: u32,
}

impl Client {
    async fn connect(server_addr: &str, password: &str) -> Result<Self, Box<dyn Error>> {
        let stream = TcpStream::connect(server_addr).await?;
        let auth_token = generate_auth_token(password); // Use parameter

        let mut client = Self {
            stream,
            auth_token,
            request_id: 1,
        };

        client.authenticate().await?;
        Ok(client)
    }

    async fn send_message(&mut self, message: &Message) -> Result<(), Box<dyn Error>> {
        let bytes = message.to_bytes();
        self.stream.write_all(&bytes).await?;
        Ok(())
    }

    async fn receive_message(&mut self) -> Result<Message, Box<dyn Error>> {
        let mut len_bytes = [0u8; 4];
        self.stream.read_exact(&mut len_bytes).await?;
        let length = u32::from_be_bytes(len_bytes);

        let mut data = vec![0u8; length as usize];
        self.stream.read_exact(&mut data).await?;

        Ok(Message::from_bytes(length, &data)?)
    }

    async fn authenticate(&mut self) -> Result<(), Box<dyn Error>> {
        let mut auth_msg = Message::new_with_auth(Operation::Auth, Vec::new(), self.auth_token);
        auth_msg.set_request_id(self.request_id);
        self.request_id += 1;

        self.send_message(&auth_msg).await?;
        let response = self.receive_message().await?;

        if response.status != StatusCode::Success {
            return Err("Authentication failed".into());
        }

        Ok(())
    }

    async fn upload_file(
        &mut self,
        local_path: &str,
        remote_name: &str,
    ) -> Result<(), Box<dyn Error>> {
        let data = fs::read(local_path)?;
        let total_size = data.len();
        let total_chunks = (total_size + CHUNK_SIZE - 1) / CHUNK_SIZE;

        println!(
            "Uploading {} as '{}' ({} bytes)",
            local_path, remote_name, total_size
        );

        for chunk_num in 0..total_chunks {
            let start = chunk_num * CHUNK_SIZE;
            let end = std::cmp::min(start + CHUNK_SIZE, total_size);
            let chunk_data = data[start..end].to_vec();

            let chunk_meta = ChunkMetadata {
                filename: remote_name.to_string(),
                chunk_number: chunk_num as u32,
                total_chunks: total_chunks as u32,
                data: chunk_data,
            };

            let mut msg = Message::new_with_auth(
                Operation::StoreChunk,
                chunk_meta.to_payload(),
                self.auth_token,
            );
            msg.set_request_id(self.request_id);
            self.request_id += 1;

            self.send_message(&msg).await?;
            let response = self.receive_message().await?;

            if response.status != StatusCode::Success {
                return Err(format!("Chunk {} upload failed", chunk_num).into());
            }

            let progress = ((chunk_num + 1) as f64 / total_chunks as f64 * 100.0) as u32;
            print!("\rProgress: {}%", progress);
            std::io::Write::flush(&mut std::io::stdout())?;
        }

        println!();

        let mut complete_msg = Message::new_with_auth(
            Operation::StoreComplete,
            remote_name.as_bytes().to_vec(),
            self.auth_token,
        );
        complete_msg.set_request_id(self.request_id);
        self.request_id += 1;

        self.send_message(&complete_msg).await?;
        let response = self.receive_message().await?;

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

    async fn download_file(
        &mut self,
        remote_name: &str,
        local_path: &str,
    ) -> Result<(), Box<dyn Error>> {
        let mut retrieve_msg = Message::new_with_auth(
            Operation::Retrieve,
            remote_name.as_bytes().to_vec(),
            self.auth_token,
        );
        retrieve_msg.set_request_id(self.request_id);
        self.request_id += 1;

        println!("Downloading '{}'...", remote_name);

        self.send_message(&retrieve_msg).await?;
        let response = self.receive_message().await?;

        if response.status != StatusCode::Success {
            return Err(format!(
                "Download failed: {}",
                String::from_utf8_lossy(&response.payload)
            )
            .into());
        }

        fs::write(local_path, &response.payload)?;

        println!(
            "✓ Downloaded to '{}' ({} bytes)",
            local_path,
            response.payload.len()
        );
        Ok(())
    }

    async fn list_files(&mut self) -> Result<(), Box<dyn Error>> {
        let mut list_msg = Message::new_with_auth(Operation::List, Vec::new(), self.auth_token);
        list_msg.set_request_id(self.request_id);
        self.request_id += 1;

        self.send_message(&list_msg).await?;
        let response = self.receive_message().await?;

        if response.status != StatusCode::Success {
            return Err("List failed".into());
        }

        let files = String::from_utf8_lossy(&response.payload);
        if files.trim().is_empty() {
            println!("No files on server");
        } else {
            println!("Files on server:");
            for file in files.lines() {
                println!("  - {}", file);
            }
        }

        Ok(())
    }

    async fn delete_file(&mut self, remote_name: &str) -> Result<(), Box<dyn Error>> {
        let mut delete_msg = Message::new_with_auth(
            Operation::Delete,
            remote_name.as_bytes().to_vec(),
            self.auth_token,
        );
        delete_msg.set_request_id(self.request_id);
        self.request_id += 1;

        self.send_message(&delete_msg).await?;
        let response = self.receive_message().await?;

        if response.status == StatusCode::Success {
            println!("✓ Deleted '{}'", remote_name);
            Ok(())
        } else {
            Err(format!(
                "Delete failed: {}",
                String::from_utf8_lossy(&response.payload)
            )
            .into())
        }
    }
}

pub async fn upload(
    server_addr: &str,
    local_path: &str,
    remote_name: Option<&str>,
    password: &str, // NEW PARAMETER
) -> Result<(), Box<dyn Error>> {
    print!("Connecting to {}...  ", server_addr);
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut client = Client::connect(server_addr, password).await?; // Pass password
    println!("✓\n");

    let filename = remote_name.unwrap_or_else(|| {
        std::path::Path::new(local_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("uploaded_file")
    });

    client.upload_file(local_path, filename).await
}

pub async fn download(
    server_addr: &str,
    remote_name: &str,
    local_path: Option<&str>,
    password: &str, // NEW PARAMETER
) -> Result<(), Box<dyn Error>> {
    print!("Connecting to {}... ", server_addr);
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut client = Client::connect(server_addr, password).await?; // Pass password
    println!("✓\n");

    let output_path = local_path.unwrap_or(remote_name);
    client.download_file(remote_name, output_path).await
}

pub async fn list(server_addr: &str, password: &str) -> Result<(), Box<dyn Error>> {
    // NEW PARAMETER
    print!("Connecting to {}... ", server_addr);
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut client = Client::connect(server_addr, password).await?; // Pass password
    println!("✓\n");

    client.list_files().await
}

pub async fn delete(
    server_addr: &str,
    remote_name: &str,
    password: &str,
) -> Result<(), Box<dyn Error>> {
    // NEW PARAMETER
    print!("Connecting to {}... ", server_addr);
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut client = Client::connect(server_addr, password).await?; // Pass password
    println!("✓\n");

    client.delete_file(remote_name).await
}
