use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Error, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct Storage {
    root_dir: PathBuf,
    pending_chunks: Mutex<HashMap<String, ChunkedUpload>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)] // For easier debugging
pub struct FileMetadata {
    pub filename: String,
    pub size: u64,
    pub last_modified: String,
    pub checksum: String, // Optional
}

struct ChunkedUpload {
    chunks: HashMap<u32, Vec<u8>>,
    total_chunks: u32,
}

impl ChunkedUpload {
    fn new(total_chunks: u32) -> Self {
        Self {
            chunks: HashMap::new(),
            total_chunks,
        }
    }

    fn add_chunk(&mut self, chunk_number: u32, data: Vec<u8>) {
        self.chunks.insert(chunk_number, data);
    }

    fn is_complete(&self) -> bool {
        self.chunks.len() == self.total_chunks as usize
    }

    fn assemble(&self) -> io::Result<Vec<u8>> {
        if !self.is_complete() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Not all chunks received",
            ));
        }

        let mut result = Vec::new();
        for i in 0..self.total_chunks {
            if let Some(chunk) = self.chunks.get(&i) {
                result.extend_from_slice(chunk);
            } else {
                return Err(Error::new(ErrorKind::InvalidData, "Missing chunk"));
            }
        }

        Ok(result)
    }
}

impl Storage {
    pub fn new(root_dir: impl AsRef<Path>) -> io::Result<Self> {
        let root = root_dir.as_ref().to_path_buf();

        // Create storage directory if it doesn't exist
        if !root.exists() {
            fs::create_dir_all(&root)?;
        }

        Ok(Self {
            root_dir: root,
            pending_chunks: Mutex::new(HashMap::new()),
        })
    }

    pub fn store_chunk(
        &self,
        filename: &str,
        chunk_number: u32,
        total_chunks: u32,
        data: Vec<u8>,
    ) -> io::Result<bool> {
        // Validate filename
        if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
            return Err(Error::new(ErrorKind::InvalidInput, "Invalid filename"));
        }

        let mut pending = self.pending_chunks.lock().unwrap();

        let upload = pending
            .entry(filename.to_string())
            .or_insert_with(|| ChunkedUpload::new(total_chunks));

        upload.add_chunk(chunk_number, data);

        Ok(upload.is_complete())
    }

    pub fn complete_chunked_upload(&self, filename: &str) -> io::Result<()> {
        let mut pending = self.pending_chunks.lock().unwrap();

        if let Some(upload) = pending.remove(filename) {
            let data = upload.assemble()?;
            drop(pending); // Release lock before file I/O

            self.store(filename, &data)?;
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::NotFound,
                "No pending upload for this file",
            ))
        }
    }

    pub fn store(&self, filename: &str, data: &[u8]) -> io::Result<()> {
        // Validate filename (prevent path traversal)
        if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
            return Err(Error::new(ErrorKind::InvalidInput, "Invalid filename"));
        }

        let file_path = self.root_dir.join(filename);
        fs::write(file_path, data)?;
        Ok(())
    }

    pub fn retrieve(&self, filename: &str) -> io::Result<Vec<u8>> {
        // Validate filename
        if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
            return Err(Error::new(ErrorKind::InvalidInput, "Invalid filename"));
        }

        let file_path = self.root_dir.join(filename);

        if !file_path.exists() {
            return Err(Error::new(ErrorKind::NotFound, "File not found"));
        }

        fs::read(file_path)
    }

    pub fn delete(&self, filename: &str) -> io::Result<()> {
        // Validate filename
        if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
            return Err(Error::new(ErrorKind::InvalidInput, "Invalid filename"));
        }

        let file_path = self.root_dir.join(filename);

        if !file_path.exists() {
            return Err(Error::new(ErrorKind::NotFound, "File not found"));
        }

        fs::remove_file(file_path)
    }

    pub fn list(&self) -> io::Result<Vec<FileMetadata>> {
        let mut result = Vec::new();

        for entry in fs::read_dir(&self.root_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                // Filename
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                // File size and modified time
                let metadata = entry.metadata()?;
                let size = metadata.len();

                let time = metadata
                    .modified()
                    .unwrap_or_else(|_| std::time::SystemTime::UNIX_EPOCH);
                let datetime: chrono::DateTime<chrono::Utc> = time.into();
                let last_modified = datetime.format("%Y-%m-%d %H:%M:%S").to_string();

                // SHA256 checksum
                let mut file = fs::File::open(&path)?;
                let mut hasher = Sha256::new();
                let mut buffer = [0; 8192];
                loop {
                    let n = file.read(&mut buffer)?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                let checksum = format!("{:x}", hasher.finalize());

                result.push(FileMetadata {
                    filename,
                    size,
                    last_modified,
                    checksum,
                });
            }
        }

        result.sort_by(|a, b| a.filename.cmp(&b.filename));
        Ok(result)
    }
}
