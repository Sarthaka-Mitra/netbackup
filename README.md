# NetBackup

**NetBackup** is a private file storage server written in Rust. It allows you to create a storage solution accessible over your local network, using a custom network protocol.

## Features
- **Custom Protocol**: Communication over a streamlined, binary protocol designed for file storage.
- **Authentication**: Access secured using SHA-256-based authentication tokens.
- **Chunked File Transfers**: Support for large file uploads and downloads in chunks.
- **Cross-Platform Support**: Runs on Linux, macOS, and Windows.
- **Command-Line Interface (CLI)**: Manage uploads, downloads, file listings, and deletions via simple commands.

## Installation
1. Install Rust (https://rustup.rs).
2. Clone this repository:
   ```bash
   git clone https://github.com/Sarthaka-Mitra/netbackup.git
   cd netbackup
   ```
3. Build and install the CLI:
   ```bash
   cargo install --path .
   ```

After installation, the `netbackup` command will be globally available.

## Usage
Start the server:
```bash
netbackup server --bind 0.0.0.0:8080
```

Upload a file:
```bash
netbackup upload myfile.txt --server 127.0.0.1:8080
```

List files:
```bash
netbackup list --server 127.0.0.1:8080
```

Download a file:
```bash
netbackup download myfile.txt --server 127.0.0.1:8080
```

Delete a file:
```bash
netbackup delete myfile.txt --server 127.0.0.1:8080
```

## How It Works
NetBackup uses a straightforward client-server architecture:
- The **server** stores and manages files in a configured directory.
- The **client** connects over a TCP socket, sends commands, and exchanges data chunks securely.

Each network operation (upload, download, etc.) is wrapped in an authenticated protocol message, ensuring data integrity and security over the local network.

## Contributing
Contributions and feedback are welcome! Feel free to open issues or submit pull requests.

---
*Built with Rust to simplify private network storage.*
