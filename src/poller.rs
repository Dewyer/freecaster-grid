use crate::{Config, NodeConfig, ObituaryResponse, StatusResponse};
use anyhow::Result;
use rand::Rng;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEAD_AFTER: usize = 3;

pub struct StateInner {
    pub failing_nodes: Vec<FailingNodeState>,
}

#[derive(Clone)]
pub struct State(Arc<Mutex<StateInner>>);

impl Deref for State {
    type Target = Mutex<StateInner>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl State {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(StateInner {
            failing_nodes: vec![],
        })))
    }
}

#[derive(Clone, Debug)]
pub struct DeadConfirmation {
    pub confirmed_roll: Option<usize>,
}

#[derive(Clone)]
pub struct FailingNodeState {
    pub name: String,
    pub last_fail: Duration,
    pub fail_count: usize,
    pub confirmations: HashMap<String, DeadConfirmation>,
    pub announcement_rolls: HashMap<String, usize>,
    pub local_announcement_roll: Option<usize>,
    pub announced: bool,
}

impl FailingNodeState {
    pub fn new_failed(name: String, last_fail: Duration) -> Self {
        Self {
            name,
            last_fail,
            fail_count: 1,
            confirmations: Default::default(),
            announcement_rolls: Default::default(),
            local_announcement_roll: None,
            announced: false,
        }
    }

    pub fn is_dead(&self) -> bool {
        self.fail_count >= DEAD_AFTER
    }

    pub fn reset(&mut self) {
        self.fail_count = 0;
        self.confirmations.clear();
        self.announcement_rolls.clear();
        self.local_announcement_roll = None;
        self.announced = false;
    }
}

pub async fn poller(poller_config: Arc<Config>, cert: &[u8], state: State) -> Result<()> {
    println!("Starting poller `{}`", poller_config.name);

    let client = Client::builder()
        .use_rustls_tls()
        .add_root_certificate(reqwest::Certificate::from_pem(cert)?)
        .danger_accept_invalid_certs(true)
        .build()?;

    loop {
        let time = SystemTime::now().duration_since(UNIX_EPOCH)?;

        println!("Polling nodes @`{time:?}`");
        let mut poll_res = HashMap::new();
        for node in poller_config.nodes.iter() {
            println!("Checking node {}: {}", node.name, node.address);
            let res = poll_node(&client, &poller_config.name, node).await;
            poll_res.insert(node.clone(), res);
        }

        let dead_copies = {
            let mut gr = state.lock().expect("Failed to lock state");
            for (node, res) in poll_res {
                let fail_state = gr.failing_nodes.iter_mut().find(|fs| fs.name == node.name);

                if res.failing {
                    if let Some(fail_state) = fail_state {
                        if !fail_state.is_dead() {
                            fail_state.fail_count += 1;
                            if fail_state.is_dead() {
                                let roll = rand::rng().random_range(0usize..usize::MAX);
                                fail_state.local_announcement_roll = Some(roll);
                                println!(
                                    "Node `{}` is dead my roll: `{}`, last fail: {:?}",
                                    node.name, roll, fail_state.last_fail
                                );
                            }
                        }
                    } else {
                        gr.failing_nodes
                            .push(FailingNodeState::new_failed(node.name.clone(), time));
                    }
                } else if let Some(fail_state) = fail_state {
                    // back up
                    if fail_state.is_dead() {
                        fail_state.reset();
                        println!("Node `{}` is back up", node.name);
                    }
                }
            }

            gr.failing_nodes
                .iter()
                .filter_map(|fs| fs.is_dead().then_some(fs.clone()))
                .collect::<Vec<_>>()
        };

        // check deaths
        let mut obi_response = HashMap::new();

        // any dead nodes need announcement
        if dead_copies.iter().any(|fs| fs.is_dead() && !fs.announced) {
            for node in poller_config.nodes.iter() {
                if dead_copies.iter().any(|fs| fs.name == node.name) {
                    continue;
                }

                let Some(orb) = call_obituary(&client, &poller_config.name, node).await else {
                    println!("Failed to call Obituary for node `{}`", node.name);
                    continue;
                };

                obi_response.insert(node.name.clone(), orb);
            }
        }

        let announcements = {
            // process obi responses
            let mut gr = state.lock().expect("Failed to lock state");
            for (from, orb) in obi_response {
                for dead_resp in orb.dead_nodes {
                    if let Some(fs) = gr
                        .failing_nodes
                        .iter_mut()
                        .find(|fs| fs.name == dead_resp.name && fs.is_dead())
                    {
                        println!("Node `{}` is confirmed dead by `{from}`", dead_resp.name);
                        fs.confirmations.insert(
                            from.clone(),
                            DeadConfirmation {
                                confirmed_roll: Some(dead_resp.roll),
                            },
                        );
                    }
                }

                // if node didnt confirm death we mark as failed confirmation of all our dead
                for fs in gr.failing_nodes.iter_mut() {
                    if !fs.is_dead() {
                        continue;
                    }

                    if !fs.confirmations.contains_key(&from) {
                        fs.confirmations.insert(
                            from.clone(),
                            DeadConfirmation {
                                confirmed_roll: None,
                            },
                        );
                    }
                }
            }

            // check death quorum and rolls
            let mut announcements = vec![];

            for fs in gr.failing_nodes.iter_mut() {
                if !fs.is_dead() {
                    continue;
                }
                if fs.announced {
                    continue;
                }

                let Some(my_roll) = fs.local_announcement_roll else {
                    continue;
                };

                let true_confirmations = fs
                    .confirmations
                    .iter()
                    .filter(|(_, val)| val.confirmed_roll.is_some())
                    .count();
                let false_confirmations = fs
                    .confirmations
                    .iter()
                    .filter(|(_, val)| val.confirmed_roll.is_none())
                    .count();
                if true_confirmations > false_confirmations {
                    println!("Node `{}` is confirmed dead by quorum", fs.name);
                }

                let mut confirmations_rolls = fs
                    .confirmations
                    .iter()
                    .filter_map(|(from, val)| val.confirmed_roll.map(|roll| (from.clone(), roll)))
                    .collect::<Vec<_>>();
                confirmations_rolls.push((poller_config.name.clone(), my_roll));
                confirmations_rolls.sort_by_key(|(_, roll)| *roll);

                let winner = confirmations_rolls.last().expect("no confirmations?");
                if winner.0 == poller_config.name {
                    println!(
                        "Node `{}`'s death to be announced by this node death rolled: {}",
                        fs.name, winner.1
                    );
                    announcements.push(fs.name.clone());
                }
                fs.announced = true; // announced death
            }

            announcements
        };

        for anc in announcements {
            announce_death_telegram(&poller_config.name, &anc, &poller_config).await;
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

struct NodeResult {
    failing: bool,
}

async fn make_whatever_logged_http_call<T: DeserializeOwned>(
    client: &Client,
    me: &str,
    node: &NodeConfig,
    endpoint: &str,
    purpose: &str,
) -> Result<Option<T>> {
    match client
        .get(format!("{}{}", node.address, endpoint))
        .header(
            "User-Agent",
            format!("freecaster-grid/{}/{}", env!("CARGO_PKG_VERSION"), me,),
        )
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                let Some(correct_response) = response
                    .json::<T>()
                    .await
                    .inspect_err(|err| {
                        println!("Failed to parse response for `{purpose}`: {err:?}");
                    })
                    .ok()
                else {
                    return Ok(None);
                };

                println!(
                    "Node `{}` returned a fine response for `{purpose}`",
                    node.name
                );
                Ok(Some(correct_response))
            } else {
                println!(
                    "Node `{}` returned error status: {}",
                    node.name,
                    response.status()
                );
                Err(anyhow::anyhow!(
                    "Node returned error status: {}",
                    response.status()
                ))
            }
        }
        Err(e) => {
            println!("Failed to connect to node {}: {:?}", node.name, e);
            Err(e.into())
        }
    }
}

async fn poll_node(client: &Client, me: &str, node: &NodeConfig) -> NodeResult {
    match make_whatever_logged_http_call::<StatusResponse>(client, me, node, "/", "poll status")
        .await
    {
        Ok(Some(correct_response)) => {
            println!(
                "Node `{}`@`{}` is up",
                correct_response.name, correct_response.version
            );
            if node.name != correct_response.name {
                println!(
                    "Node name mismatch: `{}` != `{}`",
                    node.name, correct_response.name
                );
            }

            NodeResult { failing: false }
        }
        Ok(None) => {
            println!("Node `{}` is up but weird", node.name);

            NodeResult { failing: false }
        }
        Err(_) => NodeResult { failing: true },
    }
}

async fn call_obituary(client: &Client, me: &str, node: &NodeConfig) -> Option<ObituaryResponse> {
    make_whatever_logged_http_call::<ObituaryResponse>(client, me, node, "/obituary", "obituary")
        .await
        .ok()
        .flatten()
}

async fn announce_death_telegram(me: &str, dead: &str, config: &Arc<Config>) {
    let res = telegram_notifyrs::send_message(
        format!("Node: `{dead}` has unfortunately died, announced by: `{me}`"),
        &config.telegram_token,
        config.telegram_chat_id,
    );
    if res.error() {
        println!("Telegram notification failed: {}", res.status());
    }
}
