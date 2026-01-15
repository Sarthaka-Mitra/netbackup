mod client;
mod protocol;
mod server;
mod storage;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "netbackup")]
#[command(about = "Network backup and storage system", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the storage server
    Server {
        /// Address to bind to (default: 0.0.0.0:8080)
        #[arg(short, long, default_value = "0.0.0.0:8080")]
        bind: String,

        /// Storage directory (default: ./storage_data)
        #[arg(short, long, default_value = "./storage_data")]
        storage: String,
    },

    /// Upload a file to the server
    Upload {
        /// Local file path
        local_file: String,

        /// Remote filename (optional)
        remote_name: Option<String>,

        /// Server address (default: 127.0.0.1:8080)
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        server: String,
    },

    /// Download a file from the server
    Download {
        /// Remote filename
        remote_file: String,

        /// Local path to save (optional)
        local_path: Option<String>,

        /// Server address (default: 127.0.0.1:8080)
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        server: String,
    },

    /// List all files on the server
    List {
        /// Server address (default: 127.0.0.1:8080)
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        server: String,
    },

    /// Delete a file from the server
    Delete {
        /// Remote filename
        remote_file: String,

        /// Server address (default: 127.0.0.1:8080)
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        server: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Server { bind, storage } => {
            server::run(bind, storage).await?;
        }
        Commands::Upload {
            local_file,
            remote_name,
            server,
        } => {
            client::upload(&server, &local_file, remote_name.as_deref()).await?;
        }
        Commands::Download {
            remote_file,
            local_path,
            server,
        } => {
            client::download(&server, &remote_file, local_path.as_deref()).await?;
        }
        Commands::List { server } => {
            client::list(&server).await?;
        }
        Commands::Delete {
            remote_file,
            server,
        } => {
            client::delete(&server, &remote_file).await?;
        }
    }

    Ok(())
}
