use std::env;
use rouille::{router, Server};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{Context, Result};
use tokio::fs;
use tokio::task::JoinSet;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub cert_path: String,
    pub key_path: String,
}

#[derive(Debug, Deserialize)]
pub struct NodeConfig {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub name: String,
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

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting freecaster-grid v{VERSION}");
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <config_path>", args[0]);
        std::process::exit(1);
    }

    // Load and parse config
    let config_path = PathBuf::from(&args[1]);
    let config = Arc::new(load_config(config_path).await?);

    println!("Loaded configuration: {:?}", config);
    let cert = fs::read(&config.server.cert_path).await.with_context(|| "Failed to read certificate")?;
    let key = fs::read(&config.server.key_path).await.with_context(|| "Failed to read key")?;

    let mut js = JoinSet::new();
    let server_config = config.clone();

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

                (GET) (/hello/world) => {
                    // If the request's URL is `/hello/world`, we jump here.
                    println!("hello world");

                    // Builds a `Response` object that contains the "hello world" text.
                    rouille::Response::text("hello world")
                },
                _ => rouille::Response::empty_404()
            )
        }, cert, key)
            .expect("Failed to start server")
            .run()
    });

    let poller_config = config.clone();
    js.spawn(async move {
        let poller = async move || -> Result<()> {
            println!("Starting poller `{}`", poller_config.name);

            loop {
                let time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)?;

                println!("Polling nodes @`{time:?}`");
                for node in poller_config.nodes.iter() {

                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        };

        poller().await.expect("Poller failed");
    });

    js.join_all().await;
    Ok(())
}
