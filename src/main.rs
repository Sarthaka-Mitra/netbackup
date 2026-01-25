mod client;
mod config;
mod protocol;
mod server;
mod storage;

use clap::{Parser, Subcommand};
use config::Config;
use crossterm::{
    event::{read, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::io::{self, Write};
use std::path::PathBuf;

fn prompt_password_masked() -> Result<String, io::Error> {
    print!("Password: ");
    io::stdout().flush()?;

    // Enable raw mode to read individual keystrokes
    enable_raw_mode()?;

    let mut password = String::new();

    loop {
        // Read a single event
        let event = read().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        match event {
            Event::Key(key_event) => {
                match key_event.code {
                    KeyCode::Enter => {
                        // User pressed Enter - we're done
                        disable_raw_mode()?;
                        println!(); // Move to next line
                        return Ok(password);
                    }

                    KeyCode::Backspace => {
                        // Handle backspace - remove last character
                        if !password.is_empty() {
                            password.pop();
                            // Move cursor back, overwrite with space, move back again
                            print!("\x08 \x08");
                            io::stdout().flush()?;
                        }
                    }

                    KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Handle Ctrl+C - exit gracefully
                        disable_raw_mode()?;
                        println!();
                        std::process::exit(0);
                    }

                    KeyCode::Char(c) => {
                        // Regular character - add to password and print asterisk
                        password.push(c);
                        print!("*");
                        io::stdout().flush()?;
                    }

                    _ => {
                        // Ignore other keys
                    }
                }
            }
            _ => {
                // Ignore other events (mouse, resize, etc.)
            }
        }
    }
}
// Helper to get password:  CLI flag > prompt
fn get_password(cli_password: Option<String>) -> String {
    match cli_password {
        Some(p) => p,
        None => prompt_password_masked().unwrap_or_else(|e| {
            // Make sure raw mode is disabled on error
            let _ = disable_raw_mode();
            eprintln!("Error reading password:  {}", e);
            std::process::exit(1);
        }),
    }
}

#[derive(Parser)]
#[command(name = "netbackup")]
#[command(about = "Network backup and storage system", long_about = None)]
struct Cli {
    /// Path to config file (optional, auto-detected if not specified)
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the storage server
    Server {
        /// Address to bind to (overrides config) [default: from config]
        #[arg(short, long)]
        bind: Option<String>,
        /// Storage directory path (overrides config) [default: from config]
        #[arg(short, long)]
        storage: Option<String>,
    },
    /// Upload a file to the server
    Upload {
        /// <local_file> - Path to the local file to upload
        local_file: String,
        /// [remote_name] - Optional remote filename (defaults to local filename)
        remote_name: Option<String>,
        /// Server address (overrides config) [default: from config]
        #[arg(short, long)]
        server: Option<String>,
        /// Password for authentication (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Download a file from the server
    Download {
        /// <remote_file> - Filename on the remote server
        remote_file: String,
        /// [local_path] - Optional local path to save (defaults to remote filename)
        local_path: Option<String>,
        /// Server address (overrides config) [default: from config]
        #[arg(short, long)]
        server: Option<String>,
        /// Password for authentication (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// List all files on the server
    List {
        /// Server address (overrides config) [default: from config]
        #[arg(short, long)]
        server: Option<String>,
        /// Password for authentication (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Delete a file from the server
    Delete {
        /// <remote_file> - Filename to delete from server
        remote_file: String,
        /// Server address (overrides config) [default: from config]
        #[arg(short, long)]
        server: Option<String>,
        /// Password for authentication (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Connect to server in interactive mode
    Connect {
        /// Server address (overrides config) [default: from config]
        #[arg(short, long)]
        server: Option<String>,
        /// Password for authentication (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Generate a default configuration file
    InitConfig {
        /// [output] - Path to write config file [default: ./netbackup.toml]
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Load configuration
    let config = match &cli.config {
        Some(path) => Config::load_from_path(path).unwrap_or_else(|e| {
            eprintln!(
                "[ERROR] Failed to load config from {}: {}",
                path.display(),
                e
            );
            std::process::exit(1);
        }),
        None => Config::load(),
    };

    match cli.command {
        Commands::Server { bind, storage } => {
            // CLI args override config file values
            let bind_addr = bind.unwrap_or(config.server.bind_address);
            let storage_path = storage.unwrap_or(config.server.storage_path);

            server::run(bind_addr, storage_path, config.auth.password).await?;
        }

        Commands::Upload {
            local_file,
            remote_name,
            server,
            password,
        } => {
            let server_addr = server.unwrap_or(config.client.default_server);
            let pass = get_password(password);
            client::upload(&server_addr, &local_file, remote_name.as_deref(), &pass).await?;
        }

        Commands::Download {
            remote_file,
            local_path,
            server,
            password,
        } => {
            let server_addr = server.unwrap_or(config.client.default_server);
            let pass = get_password(password);
            client::download(&server_addr, &remote_file, local_path.as_deref(), &pass).await?;
        }

        Commands::List { server, password } => {
            let server_addr = server.unwrap_or(config.client.default_server);
            let pass = get_password(password);
            client::list(&server_addr, &pass).await?;
        }

        Commands::Delete {
            remote_file,
            server,
            password,
        } => {
            let server_addr = server.unwrap_or(config.client.default_server);
            let pass = get_password(password);
            client::delete(&server_addr, &remote_file, &pass).await?;
        }
        Commands::Connect { server, password } => {
            let server_addr = server.unwrap_or(config.client.default_server);
            let pass = get_password(password);
            client::interactive_session(&server_addr, &pass).await?;
        }

        Commands::InitConfig { output } => {
            let path = output.unwrap_or_else(|| PathBuf::from("netbackup.toml"));

            if path.exists() {
                eprintln!("[ERROR] Config file already exists at:  {}", path.display());
                eprintln!("        Use a different path or delete the existing file.");
                std::process::exit(1);
            }

            Config::generate_default(&path)?;
            println!("[SUCCESS] Config file created!");
        }
    }

    Ok(())
}
