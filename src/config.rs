use anyhow::{Context, Result};
use config::Case;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
pub struct ServerConfig {
    #[serde(default = "default_ip_address")]
    pub ip_address: String,
    pub port: u16,
    #[serde(default)]
    pub ssl: Option<SSLConfig>,
}

fn default_ip_address() -> String {
    "0.0.0.0".into()
}

#[derive(Debug, Deserialize, Clone)]
#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
pub struct SSLConfig {
    pub cert_path: String,
    pub key_path: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone, Serialize)]
#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
pub struct NodeConfig {
    #[serde(default)]
    pub telegram_handle: Option<String>,
    pub address: String,
}

impl NodeConfig {
    pub fn with_name<'a>(&'a self, name: &'a String) -> NamedNodeConfig<'a> {
        NamedNodeConfig { name, config: self }
    }
}

pub struct NamedNodeConfig<'a> {
    pub name: &'a String,
    pub config: &'a NodeConfig,
}

#[derive(Debug, Deserialize, Default, Clone, Copy, Serialize)]
#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AnnouncementMode {
    #[default]
    Telegram,
    Log,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
pub struct TelegramConfig {
    pub token: String,
    pub chat_id: i64,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "json_schema", derive(schemars::JsonSchema))]
pub struct Config {
    pub name: String,
    #[serde(default)]
    pub telegram: Option<TelegramConfig>,
    pub secret_key: String,
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    #[cfg_attr(feature = "json_schema", schemars(with = "String"))]
    pub poll_time: Option<std::time::Duration>,

    #[serde(default)]
    pub announcement_mode: AnnouncementMode,

    pub server: ServerConfig,

    #[serde(default)]
    pub nodes: HashMap<String, NodeConfig>,

    #[serde(default)]
    pub webui_enabled: bool,
}

pub async fn load_config(path: Option<PathBuf>) -> Result<Config> {
    let config = config::Config::builder();
    let config = if let Some(path) = path {
        config.add_source(config::File::from(path.clone()))
    } else {
        config
    };
    let config = config
        .add_source(
            config::Environment::with_prefix("FC")
                .prefix_separator("_")
                .separator("__")
                .convert_case(Case::Snake),
        )
        .build()
        .context("Failed to build config")?
        .try_deserialize()
        .context("Failed to deserialize config")?;

    Ok(config)
}
