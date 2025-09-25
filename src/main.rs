mod config;
mod poller;

use crate::config::{Config, load_config};

use crate::poller::{NodeSilence, State, poller};
use anyhow::{Context, Result};
use chrono::{DateTime, Local, SubsecRound, Utc};
use env_logger::Builder;
use log::{LevelFilter, error, info, warn};
use rand::Rng;
use rouille::{Request, Server, router, try_or_400};
use serde::{Deserialize, Serialize};
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::fs;
use tokio::task::JoinSet;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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

#[derive(Debug, Serialize, Deserialize, Copy, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GridNodeStatus {
    Alive,
    Dying,
    Dead,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GridNodeResponse {
    pub name: String,
    pub last_poll: Option<DateTime<Utc>>,
    pub status: GridNodeStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GridResponse {
    pub nodes: Vec<GridNodeResponse>,

    // totals
    pub alive_nodes: usize,
    pub dead_nodes: usize,
    pub dying_nodes: usize,
    pub total_nodes: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SilenceResponse {
    pub name: String,
    pub silent_until: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SilenceBroadcastRequest {
    pub id: usize,
    pub node_name: String,
    pub silent_until: DateTime<Utc>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}]::{} - {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .init();

    info!("Starting freecaster-grid v{VERSION}");
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        warn!("Running without config file")
    }

    let config_path = if args.len() >= 2 {
        Some(PathBuf::from(&args[1]))
    } else {
        None
    };

    // Load and parse config
    let mut config = dbg!(load_config(config_path).await?);

    // filter myself out
    config.nodes.retain(|_, n| *n.name != config.name);

    let config = Arc::new(config);

    info!("Loaded configuration, this node is: {}", config.name);

    let (key, cert) = if let Some(ssl_config) = &config.server.ssl {
        let cert = fs::read(&ssl_config.cert_path)
            .await
            .with_context(|| "Failed to read certificate")?;
        let key = fs::read(&ssl_config.key_path)
            .await
            .with_context(|| "Failed to read key")?;
        (Some(key), Some(cert))
    } else {
        (None, None)
    };

    let mut js = JoinSet::new();
    let server_config = config.clone();
    let poller_cert = cert.clone();
    let state = State::new();
    let server_state = state.clone();

    js.spawn(async move {
        info!("Starting server `{}`", server_config.server.host);

        let host = server_config.server.host.clone();
        let ssl = server_config.server.ssl.is_some();

        let webui_enabled = server_config.webui_enabled;
        let router = move |request: &Request| {
            // Serve /webui and static files if enabled
            if webui_enabled {
                if request.url() == "/webui" || request.url() == "/webui/" {
                    return rouille::Response::html(include_str!("webui/index.html"));
                }
                if let Some(path) = request.url().strip_prefix("/webui/") {
                    match path {
                        "app.js" => {
                            return rouille::Response::from_data("application/javascript", include_str!("webui/app.js").as_bytes());
                        },
                        "style.css" => {
                            return rouille::Response::from_data("text/css", include_str!("webui/style.css"));
                        },
                        "freecaster.svg" => {
                            return rouille::Response::from_data("image/svg+xml", include_bytes!("webui/freecaster.svg").as_ref());
                        },
                        "freecaster-dark.svg" => {
                            return rouille::Response::from_data("image/svg+xml", include_bytes!("webui/freecaster-dark.svg").as_ref());
                        },
                        "freecaster-light.svg" => {
                            return rouille::Response::from_data("image/svg+xml", include_bytes!("webui/freecaster-light.svg").as_ref());
                        },
                        _ => {}
                    }
                }
            }
            router!(request,
                (GET) (/) => {
                    let user_agent = request.header("User-Agent").unwrap_or("Unknown");
                    info!("Called for status ua: `{user_agent}`");

                    rouille::Response::json(&StatusResponse {
                        name: server_config.name.clone(),
                        version: VERSION.to_string(),
                    })
                        .with_status_code(200)
                },

                (GET) (/obituary/{key: String}) => {
                    info!("Called for obituary");
                    if key != server_config.secret_key {
                        warn!("Invalid secret key");
                        return rouille::Response::empty_406();
                    }

                    let gr = server_state.lock().expect("Failed to lock state");
                    let dead_nodes = gr.node_state.iter().filter(|fs| fs.is_dead()).map(|fs| DeadNodeResponse {
                        name: fs.name.clone(),
                        roll: fs.local_announcement_roll.unwrap_or(usize::MAX),
                    })
                        .collect();

                    rouille::Response::json(&ObituaryResponse {
                        dead_nodes,
                    })
                        .with_status_code(200)
                },

                (POST) (/silence-broadcast/{key: String}) => {
                    info!("Called for silence broadcast");
                    if key != server_config.secret_key {
                        warn!("Invalid secret key");
                        return rouille::Response::empty_406();
                    }

                    let body: SilenceBroadcastRequest = try_or_400!(rouille::input::json_input(request));
                    let mut gr = server_state.lock().expect("Failed to lock state");
                    let found = gr.silences.iter().any(|sl| sl.id == body.id);
                    if found {
                        warn!("Silence already exists");
                        return rouille::Response::empty_204();
                    }

                    // add otherwise
                    gr.silences.push(NodeSilence {
                        id: body.id,
                        node_name: body.node_name,
                        silent_until: body.silent_until,
                        broadcasted: true,
                    });
                    rouille::Response::empty_204()
                },

                (GET) (/silence/{key: String}/{time: String}) => {
                    info!("Called for silence");
                    handle_silence(&server_config, &server_state, key, time, None)
                },

                (GET) (/silence/{key: String}/{time: String}/{target: String}) => {
                    info!("Called for silence");
                    handle_silence(&server_config, &server_state, key, time, Some(target))
                },

                (GET) (/grid/{key: String}) => {
                    info!("Called for grid");
                    if key != server_config.secret_key {
                        warn!("Invalid secret key");
                        return rouille::Response::empty_406();
                    }

                    let gr = server_state.lock().expect("Failed to lock state");
                    let mut resp = GridResponse {
                        nodes: Default::default(),
                        alive_nodes: 1,dead_nodes: 0,dying_nodes: 0,total_nodes: 1, // this node included
                    };


                    // add this node
                    resp.nodes.push(GridNodeResponse {
                        name: server_config.name.clone(),
                        last_poll: None,
                        status: GridNodeStatus::Alive,
                    });

                    for fs in gr.node_state.iter() {
                        let node_resp = fs.to_api_response();
                        match node_resp.status {
                            GridNodeStatus::Alive => {
                                resp.alive_nodes += 1;
                            },
                            GridNodeStatus::Dying => {
                                resp.dying_nodes += 1;
                            },
                            GridNodeStatus::Dead => {
                                resp.dead_nodes += 1;
                            }
                        }
                        resp.total_nodes += 1;
                        resp.nodes.push(node_resp);
                    }
                    resp.nodes.sort_by(|a, b| a.name.cmp(&b.name));

                    rouille::Response::json(&resp)
                        .with_status_code(200)
                },

                _ => rouille::Response::empty_404()
            )
        };

        if ssl {
            info!("Starting server with SSL");
            if let (Some(cert), Some(key)) = (cert, key) {
                Server::new_ssl(host, router , cert, key)
                    .expect("Failed to start server")
                    .run()
            } else {
                error!("No certificate or key provided in SSL mode, aborting");
            }
        } else {
            info!("Starting server without SSL");
            Server::new(host, router)
                .expect("Failed to start server")
                .run()
        }
    });

    let poller_config = config.clone();
    let poller_state = state.clone();

    js.spawn(async move {
        poller(poller_config, poller_cert, poller_state)
            .await
            .expect("Poller failed");
    });

    js.join_all().await;
    Ok(())
}

fn handle_silence(
    server_config: &Config,
    server_state: &State,
    key: String,
    time: String,
    target: Option<String>,
) -> rouille::Response {
    if key != server_config.secret_key {
        warn!("Invalid secret key");
        return rouille::Response::empty_406();
    }

    let mut gr = server_state.lock().expect("Failed to lock state");
    let id = rand::rng().random_range(0usize..usize::MAX);

    let Some(silent_until) = try_parse_until_time(&time) else {
        return rouille::Response::empty_400();
    };

    let target = target.unwrap_or_else(|| server_config.name.clone());

    // check if target is valid
    if !gr.node_state.iter().any(|fs| fs.name == target) && target != server_config.name {
        return rouille::Response::empty_404();
    }

    let resp = SilenceResponse {
        name: target.clone(),
        silent_until,
    };

    gr.silences.push(NodeSilence {
        id,
        node_name: target.clone(),
        silent_until,
        broadcasted: false,
    });
    info!("Added silence for {} until `{}`", target, silent_until);

    rouille::Response::json(&resp).with_status_code(200)
}

fn try_parse_until_time(time: &str) -> Option<DateTime<Utc>> {
    // try to parse as time, otherwise its duration
    if let Ok(time) = i64::from_str(time)
        && let Some(time) = DateTime::<Utc>::from_timestamp(time, 0)
    {
        return Some(time);
    }

    let duration = humantime::parse_duration(time).ok()?;
    Some(Utc::now().trunc_subsecs(0) + duration)
}
