mod poller;

use crate::poller::{State, poller};
use anyhow::{Context, Result};
use rouille::{Server, router};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::fs;
use tokio::task::JoinSet;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub cert_path: String,
    pub key_path: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Hash, Clone, Serialize)]
pub struct NodeConfig {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub name: String,
    pub telegram_token: String,
    pub telegram_chat_id: i64,

    pub server: ServerConfig,

    #[serde(default)]
    pub nodes: Vec<NodeConfig>,
}

async fn load_config(path: PathBuf) -> Result<Config> {
    println!("Loading config from {path:?}");

    let config_str = fs::read_to_string(path).await?;
    let config: Config = serde_yml::from_str(&config_str)?;
    Ok(config)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub version: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeadNodeResponse {
    pub name: String,
    pub roll: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObituaryResponse {
    pub dead_nodes: Vec<DeadNodeResponse>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    println!("Starting freecaster-grid v{VERSION}");
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <config_path>", args[0]);
        std::process::exit(1);
    }

    // Load and parse config
    let config_path = PathBuf::from(&args[1]);
    let mut config = load_config(config_path).await?;
    if let Ok(token) = env::var("TELEGRAM_TOKEN") {
        println!("Overriding telegram token with env var");
        config.telegram_token = token;
    }

    if let Ok(chat_id) = env::var("TELEGRAM_CHAT_ID") {
        println!("Overriding telegram chat id with env var");
        config.telegram_chat_id = i64::from_str(&chat_id)?;
    }

    let config = Arc::new(config);

    println!("Loaded configuration, this node is: {}", config.name);
    let cert = fs::read(&config.server.cert_path)
        .await
        .with_context(|| "Failed to read certificate")?;
    let key = fs::read(&config.server.key_path)
        .await
        .with_context(|| "Failed to read key")?;

    let mut js = JoinSet::new();
    let server_config = config.clone();
    let server_cert = cert.clone();
    let state = State::new();
    let server_state = state.clone();

    js.spawn(async move {
        println!("Starting server `{}`", server_config.server.host);

        Server::new_ssl(server_config.server.host.clone(), move |request| {
            router!(request,
                (GET) (/) => {
                    rouille::Response::json(&StatusResponse {
                        name: server_config.name.clone(),
                        version: VERSION.to_string(),
                    })
                        .with_status_code(200)
                },

                (GET) (/obituary) => {
                    println!("Called for obituary");
                    let gr = server_state.lock().expect("Failed to lock state");
                    let dead_nodes = gr.failing_nodes.iter().filter(|fs| fs.is_dead()).map(|fs| DeadNodeResponse {
                        name: fs.name.clone(),
                        roll: fs.local_announcement_roll.unwrap_or(usize::MAX),
                    })
                        .collect();

                    rouille::Response::json(&ObituaryResponse {
                        dead_nodes,
                    })
                        .with_status_code(200)
                },
                _ => rouille::Response::empty_404()
            )
        }, server_cert, key)
            .expect("Failed to start server")
            .run()
    });

    let poller_config = config.clone();
    let poller_state = state.clone();
    js.spawn(async move {
        poller(poller_config, &cert, poller_state)
            .await
            .expect("Poller failed");
    });

    js.join_all().await;
    Ok(())
}
