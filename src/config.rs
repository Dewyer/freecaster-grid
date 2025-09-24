use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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

#[derive(Debug, Deserialize)]
pub struct Config {
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
}

pub async fn load_config(path: PathBuf) -> Result<Config> {
    log::info!("Loading config from {path:?}");

    let config_str = fs::read_to_string(path).await?;
    let config: Config = serde_norway::from_str(&config_str)?;
    Ok(config)
}
