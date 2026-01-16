use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Server-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    #[serde(default = "default_storage_path")]
    pub storage_path: String,
}

/// Client-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    #[serde(default = "default_server_address")]
    pub default_server: String,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_password")]
    pub password: String,
}

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub client: ClientConfig,

    #[serde(default)]
    pub auth: AuthConfig,
}

// Default value functions
fn default_bind_address() -> String {
    "0.0.0.0:8080".to_string()
}

fn default_storage_path() -> String {
    "./storage_data".to_string()
}

fn default_server_address() -> String {
    "127.0.0.1:8080".to_string()
}

fn default_password() -> String {
    "secure_password_123".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            storage_path: default_storage_path(),
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            default_server: default_server_address(),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            password: default_password(),
        }
    }
}

impl Config {
    /// Load configuration from file, falling back to defaults if not found
    pub fn load() -> Self {
        let config_paths = Self::get_config_paths();

        for path in config_paths {
            if path.exists() {
                println!("[CONFIG] Loading from:  {}", path.display());
                match Self::load_from_path(&path) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!(
                            "[CONFIG] Warning: Failed to parse {}: {}",
                            path.display(),
                            e
                        );
                        continue;
                    }
                }
            }
        }

        println!("[CONFIG] No config file found, using defaults");
        Config::default()
    }

    /// Load configuration from a specific path
    pub fn load_from_path(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Get list of paths to search for config file (in priority order)
    fn get_config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // 1. Current directory (highest priority)
        paths.push(PathBuf::from("netbackup.toml"));

        // 2. User's config directory
        if let Some(config_dir) = directories::ProjectDirs::from("", "", "netbackup") {
            paths.push(config_dir.config_dir().join("config.toml"));
        }

        // 3. Home directory dotfile
        if let Some(home) = directories::UserDirs::new() {
            paths.push(home.home_dir().join(".netbackup.toml"));
        }

        paths
    }

    /// Generate a default config file at the specified path
    pub fn generate_default(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let default_config = Config::default();
        let toml_string = toml::to_string_pretty(&default_config)?;

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::write(path, toml_string)?;
        println!("[CONFIG] Generated default config at: {}", path.display());
        Ok(())
    }
}
