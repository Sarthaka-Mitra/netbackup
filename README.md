# Netbackup

A network-based file storage server written in Rust. NetBackup provides a private file storage solution accessible over your local network using a custom binary protocol with SHA-256 authentication and integrity verification.

## Features

- **Custom Binary Protocol**: Length-prefixed message format with operation codes, status codes, and SHA-256 checksums for data integrity
- **SHA-256 Authentication**: Token-based authentication for secure access control
- **Chunked File Transfers**: 64KB chunk-based transfers for efficient handling of large files with progress indicators
- **Interactive Mode**: Shell-like interface for managing files without repeated authentication
- **File Metadata**: Automatic SHA-256 checksum calculation and tracking of file size and modification time
- **Flexible Configuration**: Auto-detection of config files from multiple locations with CLI override support
- **Cross-Platform**: Runs on Linux, macOS, and Windows

## Installation

### Prerequisites
- Rust 1.70 or later (install from https://rustup.rs)

### Build and Install
```bash
git clone https://github.com/Sarthaka-Mitra/netbackup.git
cd netbackup
cargo install --path .
```

After installation, the `netbackup` command will be available globally.

## Configuration

### Initialize Configuration
Generate a default configuration file:
```bash
netbackup init-config
```

This creates `netbackup.toml` in the current directory with default values.

### Configuration File
NetBackup automatically searches for configuration files in the following order:
1. `./netbackup.toml` (current directory)
2. `~/.config/netbackup/config.toml` (user config directory)
3. `~/.netbackup.toml` (home directory)

Example configuration:
```toml
[server]
bind_address = "0.0.0.0:8080"
storage_path = "./storage_data"

[client]
default_server = "127.0.0.1:8080"

[auth]
password = "your_secure_password"
```

You can also specify a custom config file path:
```bash
netbackup --config /path/to/config.toml <command>
```

## Usage

### Server

Start the server:
```bash
netbackup server
```

Override configuration via CLI:
```bash
netbackup server --bind 0.0.0.0:8080 --storage /path/to/storage
```

### Client Commands

All client commands support password input via prompt (with masked input) or CLI flag.

**Upload a file:**
```bash
netbackup upload myfile.txt
netbackup upload myfile.txt remote_name.txt  # specify remote filename
```

**Download a file:**
```bash
netbackup download myfile.txt
netbackup download myfile.txt local_copy.txt  # specify local path
```

**List files:**
```bash
netbackup list
```

**Delete a file:**
```bash
netbackup delete myfile.txt
```

**Interactive mode:**
```bash
netbackup connect
```

In interactive mode, you can execute multiple commands without re-authenticating:
```
netbackup> help
netbackup> list
netbackup> upload myfile.txt
netbackup> llist           # list local files
netbackup> download file.txt
netbackup> delete old.txt
netbackup> exit
```

## Architecture

### Protocol Design

NetBackup uses a custom binary protocol over TCP with the following message structure:

```
[4 bytes: message length]
[4 bytes: request ID]
[1 byte: operation code]
[1 byte: status code]
[32 bytes: SHA-256 checksum]
[32 bytes: authentication token]
[variable: payload]
```

**Operation Codes:**
- `0x01` - Store (legacy single-message upload)
- `0x02` - Retrieve (legacy single-message download)
- `0x03` - Delete
- `0x04` - List
- `0x05` - Auth
- `0x06` - StoreChunk (upload file chunk)
- `0x07` - RetrieveChunk (download file chunk)
- `0x08` - StoreComplete (signal upload completion)

**Status Codes:**
- `0x00` - Success
- `0x01` - Error: Not Found
- `0x02` - Error: Permission Denied
- `0x03` - Error: Invalid Data
- `0x04` - Error: Server Error

### Chunked Transfer Protocol

Files are transferred in 64KB chunks to enable progress tracking and efficient memory usage.

**Upload workflow:**
1. Client sends authentication message
2. Client sends `StoreChunk` messages with chunk metadata (filename, chunk number, total chunks, data)
3. Server stores chunks in memory until all are received
4. Client sends `StoreComplete` message
5. Server assembles chunks and writes the complete file to disk

**Download workflow:**
1. Client sends authentication message
2. Client requests file metadata via `List` operation
3. Client sends `RetrieveChunk` requests with filename and chunk number
4. Server responds with chunk data and metadata
5. Client assembles chunks and writes to local file

### Security Model

- Authentication tokens are SHA-256 hashes of the configured password
- All messages include SHA-256 checksums of the payload for integrity verification
- Server validates both authentication tokens and checksums before processing requests
- Filename validation prevents path traversal attacks

## Technical Details

### Dependencies
- `tokio` - Async runtime for concurrent client handling
- `sha2` - SHA-256 hashing for authentication and checksums
- `clap` - Command-line argument parsing
- `serde` / `bincode` - Serialization for file metadata
- `toml` - Configuration file parsing
- `indicatif` - Progress bars for file transfers
- `chrono` - Timestamp formatting
- `crossterm` - Terminal control for password masking
- `directories` - Cross-platform config directory detection

### Performance Characteristics
- Chunk size: 64KB (configurable via `CHUNK_SIZE` constant in `protocol.rs`)
- Concurrent client connections supported via Tokio async runtime
- Memory-efficient streaming for large file transfers
- In-memory chunk buffering during uploads

### Limitations
- Authentication is password-based with no support for key-based auth or multi-user access control
- No encryption in transit (plaintext TCP); suitable for trusted local networks only
- No file versioning or conflict resolution
- Single flat storage directory; no subdirectory support
- Filenames cannot contain path separators or `..` sequences

## License

MIT

## Contributing

This is a personal project developed as a learning exercise in Rust systems programming and network protocol design. Feedback and suggestions are welcome via issues.
