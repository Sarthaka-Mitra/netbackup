use crate::protocol::{
    generate_auth_token, ChunkDownloadRequest, ChunkDownloadResponse, ChunkMetadata, Message,
    Operation, StatusCode, CHUNK_SIZE,
};
use indicatif::ProgressBar;
use std::error::Error;
use std::fs;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub struct Client {
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

        let pb = ProgressBar::new(total_chunks as u64);
        pb.set_message("Uploading");

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
                pb.abandon();
                return Err(format!("Chunk {} upload failed", chunk_num).into());
            }
            pb.inc(1);
        }

        pb.finish_with_message("Upload complete!");
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
            Ok(())
        } else {
            Err(format!(
                "Upload finalization failed: {}",
                String::from_utf8_lossy(&response.payload)
            )
            .into())
        }
    }

    async fn get_file_metadata(
        &mut self,
        remote_name: &str,
    ) -> Result<crate::storage::FileMetadata, Box<dyn Error>> {
        let files = self.list_files_and_return().await?;
        files
            .into_iter()
            .find(|f| f.filename == remote_name)
            .ok_or_else(|| format!("File {} not found on server", remote_name).into())
    }

    async fn list_files_and_return(
        &mut self,
    ) -> Result<Vec<crate::storage::FileMetadata>, Box<dyn Error>> {
        let mut list_msg = Message::new_with_auth(Operation::List, Vec::new(), self.auth_token);
        list_msg.set_request_id(self.request_id);
        self.request_id += 1;

        self.send_message(&list_msg).await?;
        let response = self.receive_message().await?;
        if response.status != StatusCode::Success {
            return Err("List failed".into());
        }
        let files: Vec<crate::storage::FileMetadata> = bincode::deserialize(&response.payload)?;
        Ok(files)
    }

    async fn download_file_chunked(
        &mut self,
        remote_name: &str,
        local_path: &str,
    ) -> Result<(), Box<dyn Error>> {
        let file_meta = self.get_file_metadata(remote_name).await?;
        let total_size = file_meta.size as usize;
        let total_chunks = (total_size + CHUNK_SIZE - 1) / CHUNK_SIZE;

        let pb = ProgressBar::new(total_chunks as u64);
        pb.set_message("Downloading");

        let mut output = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(local_path)?;

        for chunk_num in 0..total_chunks {
            let chunk_req = ChunkDownloadRequest {
                filename: remote_name.to_string(),
                chunk_number: chunk_num as u32,
                chunk_size: CHUNK_SIZE as u32,
            };
            let mut msg = Message::new_with_auth(
                Operation::RetrieveChunk,
                chunk_req.to_payload(),
                self.auth_token,
            );
            msg.set_request_id(self.request_id);
            self.request_id += 1;

            self.send_message(&msg).await?;
            let response = self.receive_message().await?;

            if response.status != StatusCode::Success {
                pb.abandon();
                return Err(format!(
                    "Chunk {} download failed: {}",
                    chunk_num,
                    String::from_utf8_lossy(&response.payload)
                )
                .into());
            }

            let chunk_resp = ChunkDownloadResponse::from_payload(&response.payload)?;
            output.seek(SeekFrom::Start(chunk_num as u64 * CHUNK_SIZE as u64))?;
            output.write_all(&chunk_resp.data)?;
            pb.inc(1);
        }
        pb.finish_with_message("Downloaded successfully!");
        println!();
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

        let files: Vec<crate::storage::FileMetadata> = bincode::deserialize(&response.payload)?;
        if files.is_empty() {
            println!("No files on server");
        } else {
            println!(
                "{:<35} {:>10} {:<26} {:<16}",
                "FILENAME", "SIZE", "LAST MODIFIED", "CHECKSUM"
            );
            for file in files {
                println!(
                    "{:<35} {:>10} {:<26} {:<16}",
                    file.filename,
                    file.size,
                    file.last_modified,
                    &file.checksum[..16] // Short checksum for readability
                );
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

// PUBLIC API

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
    let mut client = Client::connect(server_addr, password).await?;
    println!("✓\n");

    let output_path = local_path.unwrap_or(remote_name);
    client.download_file_chunked(remote_name, output_path).await
}

// List and delete leave unchanged
pub async fn list(server_addr: &str, password: &str) -> Result<(), Box<dyn Error>> {
    print!("Connecting to {}... ", server_addr);
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut client = Client::connect(server_addr, password).await?;
    println!("✓\n");

    client.list_files().await
}

pub async fn delete(
    server_addr: &str,
    remote_name: &str,
    password: &str,
) -> Result<(), Box<dyn Error>> {
    print!("Connecting to {}... ", server_addr);
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut client = Client::connect(server_addr, password).await?;
    println!("✓\n");

    client.delete_file(remote_name).await
}

pub async fn interactive_session(
    server_addr: &str,
    password: &str,
) -> Result<(), Box<dyn Error>> {
    print!("Connecting to {}... ", server_addr);
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut client = Client::connect(server_addr, password).await?;
    println!("✓\n");
    println!("Connected to {}. Type 'help' for commands or 'exit' to quit.\n", server_addr);
    loop {
        print!("netbackup> ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        let parts: Vec<&str> = input.split_whitespace().collect();
        let command = parts[0];
        match command {
            "exit" | "quit" => {
                println!("Disconnecting...");
                break;
            }
            "help" => {
                println!("Available commands:");
                println!("  upload <local_file> [remote_name]   - Upload a file to server");
                println!("  download <remote_file> [local_path] - Download a file from server");
                println!("  list                                - List files on remote server");
                println!("  llist [directory]                   - List local files (defaults to current directory)");
                println!("  delete <remote_file>                - Delete a file from server");
                println!("  help                                - Show this help message");
                println!("  exit | quit                         - Disconnect and quit session");
                println!();
                println!("Syntax:");
                println!("  <argument>  - Required argument");
                println!("  [argument]  - Optional argument");
            }
            "list" => {
                if let Err(e) = client.list_files().await {
                    eprintln!("Error: {}", e);
                }
            }
            "llist" => {
                let dir_path = parts.get(1).copied().unwrap_or(".");
                match std::fs::read_dir(dir_path) {
                    Ok(entries) => {
                        let mut files: Vec<_> = entries
                            .filter_map(|e| e.ok())
                            .collect();
                        files.sort_by_key(|e| e.file_name());

                        println!("{:<40} {:>12} {}", "NAME", "SIZE", "TYPE");
                        for entry in files {
                            let metadata = entry.metadata().ok();
                            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                            let file_type = if metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
                                "DIR"
                            } else {
                                "FILE"
                            };
                            println!(
                                "{:<40} {:>12} {}",
                                entry.file_name().to_string_lossy(),
                                if file_type == "DIR" { "-".to_string() } else { size.to_string() },
                                file_type
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Error listing local directory: {}", e);
                    }
                }
            }
            "upload" => {
                if parts.len() < 2 {
                    eprintln!("Usage: upload <local_file> [remote_name]");
                    continue;
                }
                let local_file = parts[1];
                let remote_name = parts.get(2).copied();
                
                let filename = remote_name.unwrap_or_else(|| {
                    std::path::Path::new(local_file)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("uploaded_file")
                });
                if let Err(e) = client.upload_file(local_file, filename).await {
                    eprintln!("Error: {}", e);
                } else {
                    println!("✓ Uploaded '{}' as '{}'", local_file, filename);
                }
            }
            "download" => {
                if parts.len() < 2 {
                    eprintln!("Usage: download <remote_file> [local_path]");
                    continue;
                }
                let remote_file = parts[1];
                let local_path = parts.get(2).copied().unwrap_or(remote_file);
                if let Err(e) = client.download_file_chunked(remote_file, local_path).await {
                    eprintln!("Error: {}", e);
                } else {
                    println!("✓ Downloaded '{}' to '{}'", remote_file, local_path);
                }
            }
            "delete" => {
                if parts.len() < 2 {
                    eprintln!("Usage: delete <remote_file>");
                    continue;
                }
                let remote_file = parts[1];
                if let Err(e) = client.delete_file(remote_file).await {
                    eprintln!("Error: {}", e);
                }
            }
            _ => {
                eprintln!("Unknown command: '{}'. Type 'help' for available commands.", command);
            }
        }
        println!(); // Add spacing between commands
    }
    Ok(())
}
