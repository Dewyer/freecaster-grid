use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{env, path::PathBuf};
use tokio::fs;

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub ssl: bool,
    #[serde(default)]
    pub cert_path: Option<String>,
    #[serde(default)]
    pub key_path: Option<String>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone, Serialize)]
pub struct NodeConfig {
    pub name: String,
    #[serde(default)]
    pub telegram_handle: Option<String>,
    pub address: String,
}

#[derive(Debug, Deserialize, Default, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnouncementMode {
    #[default]
    Telegram,
    Log,
}

/// Local instance configuration - contains personal/instance-specific settings
#[derive(Debug, Deserialize)]
pub struct LocalConfig {
    pub name: String,
    pub server: ServerConfig,
    #[serde(default)]
    pub webui_enabled: bool,
    
    // Optional grid config path for centralized configuration
    #[serde(default)]
    pub grid_config_path: Option<String>,
    #[serde(default)]
    pub grid_config_url: Option<String>,
    #[serde(default)]
    pub auto_update_grid_config: bool,
}

/// Grid configuration - contains shared settings across the grid
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GridConfig {
    pub telegram_token: String,
    pub telegram_chat_id: i64,
    pub secret_key: String,
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    pub poll_time: Option<std::time::Duration>,

    #[serde(default)]
    pub announcement_mode: AnnouncementMode,

    #[serde(default)]
    pub nodes: Vec<NodeConfig>,
}

/// Combined configuration for runtime use
#[derive(Debug)]
pub struct Config {
    pub name: String,
    pub telegram_token: String,
    pub telegram_chat_id: i64,
    pub secret_key: String,
    pub poll_time: Option<std::time::Duration>,
    pub announcement_mode: AnnouncementMode,
    pub server: ServerConfig,
    pub nodes: Vec<NodeConfig>,
    pub webui_enabled: bool,
}

/// Apply environment variable overrides to configuration values
/// Uses a systematic naming convention: PREFIX_SECTION_FIELD
fn apply_env_overrides(config: &mut Config) -> Result<()> {
    // Direct config fields
    if let Ok(name) = env::var("FREECASTER_NAME") {
        log::info!("Overriding name with env var");
        config.name = name;
    }
    
    if let Ok(token) = env::var("FREECASTER_TELEGRAM_TOKEN") {
        log::info!("Overriding telegram token with env var");
        config.telegram_token = token;
    }
    
    if let Ok(chat_id) = env::var("FREECASTER_TELEGRAM_CHAT_ID") {
        log::info!("Overriding telegram chat id with env var");
        config.telegram_chat_id = chat_id.parse()
            .with_context(|| "Invalid FREECASTER_TELEGRAM_CHAT_ID format")?;
    }
    
    if let Ok(secret) = env::var("FREECASTER_SECRET_KEY") {
        log::info!("Overriding secret key with env var");
        config.secret_key = secret;
    }
    
    if let Ok(poll_time) = env::var("FREECASTER_POLL_TIME") {
        log::info!("Overriding poll time with env var");
        config.poll_time = Some(humantime::parse_duration(&poll_time)
            .with_context(|| "Invalid FREECASTER_POLL_TIME format")?);
    }
    
    if let Ok(mode) = env::var("FREECASTER_ANNOUNCEMENT_MODE") {
        log::info!("Overriding announcement mode with env var");
        config.announcement_mode = match mode.to_lowercase().as_str() {
            "telegram" => AnnouncementMode::Telegram,
            "log" => AnnouncementMode::Log,
            _ => return Err(anyhow::anyhow!("Invalid FREECASTER_ANNOUNCEMENT_MODE: {}", mode)),
        };
    }
    
    if let Ok(webui) = env::var("FREECASTER_WEBUI_ENABLED") {
        log::info!("Overriding WebUI enabled with env var");
        config.webui_enabled = matches!(webui.as_str(), "1" | "true" | "yes" | "on");
    }
    
    // Server config overrides
    if let Ok(host) = env::var("FREECASTER_SERVER_HOST") {
        log::info!("Overriding server host with env var");
        config.server.host = host;
    }
    
    if let Ok(ssl) = env::var("FREECASTER_SERVER_SSL") {
        log::info!("Overriding server SSL with env var");
        config.server.ssl = matches!(ssl.as_str(), "1" | "true" | "yes" | "on");
    }
    
    if let Ok(cert_path) = env::var("FREECASTER_SERVER_CERT_PATH") {
        log::info!("Overriding server cert path with env var");
        config.server.cert_path = Some(cert_path);
    }
    
    if let Ok(key_path) = env::var("FREECASTER_SERVER_KEY_PATH") {
        log::info!("Overriding server key path with env var");
        config.server.key_path = Some(key_path);
    }

    Ok(())
}

/// Load grid configuration from file or URL
async fn load_grid_config(local_config: &LocalConfig) -> Result<GridConfig> {
    if let Some(path) = &local_config.grid_config_path {
        log::info!("Loading grid config from file: {path:?}");
        let config_str = fs::read_to_string(path).await
            .with_context(|| format!("Failed to read grid config from {}", path))?;
        serde_norway::from_str(&config_str)
            .with_context(|| format!("Failed to parse grid config from {}", path))
    } else if let Some(url) = &local_config.grid_config_url {
        log::info!("Loading grid config from URL: {url}");
        let client = reqwest::Client::new();
        let config_str = client.get(url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch grid config from {}", url))?
            .text()
            .await
            .with_context(|| format!("Failed to read grid config response from {}", url))?;
        serde_norway::from_str(&config_str)
            .with_context(|| format!("Failed to parse grid config from {}", url))
    } else {
        Err(anyhow::anyhow!("No grid config source specified"))
    }
}

/// Load local configuration and optionally grid configuration
pub async fn load_config(path: PathBuf) -> Result<Config> {
    log::info!("Loading local config from {path:?}");

    let config_str = fs::read_to_string(&path).await
        .with_context(|| format!("Failed to read config from {:?}", path))?;
    
    // Try to load as combined config (legacy format) first
    if let Ok(legacy_config) = serde_norway::from_str::<LegacyConfig>(&config_str) {
        log::info!("Loaded legacy combined config format");
        let mut config = Config::from_legacy(legacy_config);
        apply_env_overrides(&mut config)?;
        return Ok(config);
    }
    
    // Try to load as local config (new split format)
    let local_config: LocalConfig = serde_norway::from_str(&config_str)
        .with_context(|| format!("Failed to parse local config from {:?}", path))?;
    
    // Load grid config if specified
    let grid_config = load_grid_config(&local_config).await
        .with_context(|| "Failed to load grid configuration")?;
    
    log::info!("Loaded split config format (local + grid)");
    let mut config = Config::from_split(local_config, grid_config);
    apply_env_overrides(&mut config)?;
    
    Ok(config)
}

/// Legacy configuration format for backward compatibility
#[derive(Debug, Deserialize)]
struct LegacyConfig {
    pub name: String,
    pub telegram_token: String,
    pub telegram_chat_id: i64,
    pub secret_key: String,
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    pub poll_time: Option<std::time::Duration>,

    #[serde(default)]
    pub announcement_mode: AnnouncementMode,

    pub server: ServerConfig,

    #[serde(default)]
    pub nodes: Vec<NodeConfig>,

    #[serde(default)]
    pub webui_enabled: bool,
}

impl Config {
    /// Create config from legacy format
    fn from_legacy(legacy: LegacyConfig) -> Self {
        Self {
            name: legacy.name,
            telegram_token: legacy.telegram_token,
            telegram_chat_id: legacy.telegram_chat_id,
            secret_key: legacy.secret_key,
            poll_time: legacy.poll_time,
            announcement_mode: legacy.announcement_mode,
            server: legacy.server,
            nodes: legacy.nodes,
            webui_enabled: legacy.webui_enabled,
        }
    }
    
    /// Create config from split format
    fn from_split(local: LocalConfig, grid: GridConfig) -> Self {
        Self {
            name: local.name,
            telegram_token: grid.telegram_token,
            telegram_chat_id: grid.telegram_chat_id,
            secret_key: grid.secret_key,
            poll_time: grid.poll_time,
            announcement_mode: grid.announcement_mode,
            server: local.server,
            nodes: grid.nodes,
            webui_enabled: local.webui_enabled,
        }
    }
}
