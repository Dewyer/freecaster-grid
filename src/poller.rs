use crate::{
    AnnouncementMode, Config, GridNodeResponse, GridNodeStatus, NodeConfig, ObituaryResponse,
    SilenceBroadcastRequest, StatusResponse,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use log::{error, info, warn};
use rand::Rng;
use reqwest::{Certificate, Client};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const DEAD_AFTER: usize = 3;
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(10);

pub struct StateInner {
    pub node_state: Vec<NodeState>,
    pub silences: Vec<NodeSilence>,
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
            node_state: vec![],
            silences: vec![],
        })))
    }
}

#[derive(Clone, Debug)]
pub struct NodeSilence {
    pub id: usize,
    pub node_name: String,
    pub silent_until: DateTime<Utc>,
    pub broadcasted: bool,
}

#[derive(Clone, Debug)]
pub struct DeadConfirmation {
    pub confirmed_roll: Option<usize>,
}

#[derive(Clone)]
pub struct NodeState {
    pub name: String,
    pub last_poll: Option<DateTime<Utc>>,
    pub last_fail: Option<DateTime<Utc>>,
    pub fail_count: usize,
    pub confirmations: HashMap<String, DeadConfirmation>,
    pub announcement_rolls: HashMap<String, usize>,
    pub local_announcement_roll: Option<usize>,
    pub announced: Option<String>,
}

impl NodeState {
    pub fn new(name: String) -> Self {
        Self {
            name,
            last_poll: None,
            last_fail: None,
            fail_count: 0,
            confirmations: Default::default(),
            announcement_rolls: Default::default(),
            local_announcement_roll: None,
            announced: None,
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
        self.last_fail = None;
        self.announced = None;
    }

    pub fn to_api_response(&self) -> GridNodeResponse {
        let status = if self.is_dead() && self.announced.is_some() {
            GridNodeStatus::Dead
        } else if self.is_dead() {
            GridNodeStatus::Dying
        } else {
            GridNodeStatus::Alive
        };

        GridNodeResponse {
            name: self.name.clone(),
            last_poll: self.last_poll,
            status,
        }
    }
}

pub async fn poller(poller_config: Arc<Config>, cert: Option<Vec<u8>>, state: State) -> Result<()> {
    info!("Starting poller `{}`", poller_config.name);

    let mut client = Client::builder().use_rustls_tls();

    if let Some(cert) = cert {
        client = client.add_root_certificate(Certificate::from_pem(&cert)?);
    }

    let client = client.danger_accept_invalid_certs(true).build()?;

    // init state
    {
        let mut gr = state.lock().expect("Failed to lock state");
        for node in poller_config.nodes.iter() {
            gr.node_state.push(NodeState::new(node.name.clone()));
        }
    }

    loop {
        let time = Utc::now();

        let has_net = check_internet_connection().await;
        if !has_net {
            warn!("No internet connection, skipping poll");
            tokio::time::sleep(DEFAULT_POLL_INTERVAL).await;
            continue;
        }

        // process silences
        let silenced_nodes_clone = {
            let mut gr = state.lock().expect("Failed to lock state");
            // expire silences
            gr.silences.retain(|sl| sl.silent_until > time);

            gr.silences.clone()
        };

        // broadcast silences
        let mut broadcast_silences = vec![];
        for sl in silenced_nodes_clone.iter() {
            if sl.broadcasted {
                continue;
            }

            // broadcast
            for node in poller_config.nodes.iter() {
                let done = call_silence_broadcast(
                    &client,
                    &poller_config.name,
                    node,
                    &poller_config.secret_key,
                    sl,
                )
                .await;

                if done {
                    broadcast_silences.push(sl.clone());
                    break;
                }
            }
        }

        // set broadcast state
        {
            let mut gr = state.lock().expect("Failed to lock state");
            for sl in gr.silences.iter_mut() {
                if broadcast_silences.iter().any(|bs| bs.id == sl.id) {
                    sl.broadcasted = true;
                }
            }
        }

        info!("Polling nodes @`{time:?}`");
        let mut poll_res = HashMap::new();
        for node in poller_config.nodes.iter() {
            if silenced_nodes_clone
                .iter()
                .any(|sl| sl.node_name == node.name)
            {
                info!("Silenced node {}", node.name);
                continue;
            }

            info!("Checking node {}: {}", node.name, node.address);
            let time = Utc::now();
            let res = poll_node(&client, &poller_config.name, node).await;
            poll_res.insert(node.clone(), (res, time));
        }

        let mut up_announcements = vec![];
        let dead_copies = {
            let mut gr = state.lock().expect("Failed to lock state");
            for (node, (res, time)) in poll_res {
                let fail_state = gr
                    .node_state
                    .iter_mut()
                    .find(|fs| fs.name == node.name)
                    .expect("node");

                fail_state.last_poll = Some(time);

                if res.failing {
                    fail_state.last_fail = Some(time);

                    if !fail_state.is_dead() {
                        fail_state.fail_count += 1;
                        if fail_state.is_dead() {
                            let roll = rand::rng().random_range(0usize..usize::MAX);
                            fail_state.local_announcement_roll = Some(roll);
                            warn!(
                                "Node `{}` is dead my roll: `{}`, last fail: {:?}",
                                node.name, roll, fail_state.last_fail
                            );
                        }
                    }
                } else {
                    // back up
                    if fail_state.is_dead() {
                        if fail_state.announced == Some(poller_config.name.clone()) {
                            up_announcements.push(node.clone());
                        }
                        fail_state.reset();
                        info!("Node `{}` is back up", node.name);
                    }
                }
            }

            gr.node_state
                .iter()
                .filter_map(|fs| fs.is_dead().then_some(fs.clone()))
                .collect::<Vec<_>>()
        };

        // announce up
        for up in up_announcements {
            match poller_config.announcement_mode {
                AnnouncementMode::Telegram => {
                    announce_telegram(&poller_config.name, &up, &poller_config, false).await;
                }
                AnnouncementMode::Log => {
                    error!("Announcement!!!: `{}` is back.", up.name);
                }
            }
        }

        // check deaths
        let mut obi_response = HashMap::new();

        // any dead nodes need announcement
        if dead_copies
            .iter()
            .any(|fs| fs.is_dead() && fs.announced.is_none())
        {
            for node in poller_config.nodes.iter() {
                if dead_copies.iter().any(|fs| fs.name == node.name) {
                    continue;
                }

                let Some(orb) = call_obituary(
                    &client,
                    &poller_config.name,
                    node,
                    &poller_config.secret_key,
                )
                .await
                else {
                    error!("Failed to call Obituary for node `{}`", node.name);
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
                    let fs = gr
                        .node_state
                        .iter_mut()
                        .find(|fs| fs.name == dead_resp.name && fs.is_dead())
                        .expect("node");

                    warn!("Node `{}` is confirmed dead by `{from}`", dead_resp.name);
                    fs.confirmations.insert(
                        from.clone(),
                        DeadConfirmation {
                            confirmed_roll: Some(dead_resp.roll),
                        },
                    );
                }

                // if node didnt confirm death we mark as failed confirmation of all our dead
                for fs in gr.node_state.iter_mut() {
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

            for fs in gr.node_state.iter_mut() {
                if !fs.is_dead() {
                    continue;
                }
                if fs.announced.is_some() {
                    continue;
                }

                let Some(my_roll) = fs.local_announcement_roll else {
                    continue;
                };

                let true_confirmations = fs
                    .confirmations
                    .iter()
                    .filter(|(_, val)| val.confirmed_roll.is_some())
                    .count()
                    + 1; // plus me
                let false_confirmations = fs
                    .confirmations
                    .iter()
                    .filter(|(_, val)| val.confirmed_roll.is_none())
                    .count();
                info!(
                    "Death consideration votes: `{true_confirmations}` dead, `{false_confirmations}` live"
                );
                info!("Rolls: {:#?} (my roll: {})", fs.confirmations, my_roll);

                if true_confirmations <= false_confirmations {
                    info!("Node `{}`'s death is not confirmed by quorum", fs.name);
                    continue;
                }

                warn!("Node `{}` is confirmed dead by quorum", fs.name);
                let mut confirmations_rolls = fs
                    .confirmations
                    .iter()
                    .filter_map(|(from, val)| val.confirmed_roll.map(|roll| (from.clone(), roll)))
                    .collect::<Vec<_>>();
                confirmations_rolls.push((poller_config.name.clone(), my_roll));
                confirmations_rolls.sort_by(|(name1, roll1), (name2, roll2)| {
                    roll1.cmp(roll2).then_with(|| name1.cmp(name2))
                });

                let winner = confirmations_rolls.last().expect("no confirmations?");
                if winner.0 == poller_config.name {
                    warn!(
                        "Node `{}`'s death to be announced by this node death rolled: {}",
                        fs.name, winner.1
                    );
                    let node = poller_config
                        .nodes
                        .iter()
                        .find(|n| n.name == fs.name)
                        .expect("node");
                    announcements.push(node.clone());
                } else {
                    warn!(
                        "Node `{}`'s death to be announced by `{}` death rolled: {}",
                        fs.name, winner.0, winner.1
                    );
                }

                fs.announced = Some(winner.0.clone()); // announced death
            }

            announcements
        };

        for anc in announcements {
            match poller_config.announcement_mode {
                AnnouncementMode::Telegram => {
                    announce_telegram(&poller_config.name, &anc, &poller_config, true).await;
                }
                AnnouncementMode::Log => {
                    error!("Announcement!!!: `{}` is dead.", anc.name);
                }
            }
        }

        tokio::time::sleep(poller_config.poll_time.unwrap_or(DEFAULT_POLL_INTERVAL)).await;
    }
}

struct NodeResult {
    failing: bool,
}

async fn check_internet_connection() -> bool {
    let Ok(resp) = reqwest::get("http://clients3.google.com/generate_204").await else {
        return false;
    };
    resp.status() == reqwest::StatusCode::NO_CONTENT
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
                        error!("Failed to parse response for `{purpose}`: {err:?}");
                    })
                    .ok()
                else {
                    return Ok(None);
                };

                info!(
                    "Node `{}` returned a fine response for `{purpose}`",
                    node.name
                );
                Ok(Some(correct_response))
            } else {
                error!(
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
            error!("Failed to connect to node {}: {:?}", node.name, e);
            Err(e.into())
        }
    }
}

async fn poll_node(client: &Client, me: &str, node: &NodeConfig) -> NodeResult {
    match make_whatever_logged_http_call::<StatusResponse>(client, me, node, "/", "poll status")
        .await
    {
        Ok(Some(correct_response)) => {
            info!(
                "Node `{}`@`{}` is up",
                correct_response.name, correct_response.version
            );
            if node.name != correct_response.name {
                warn!(
                    "Node name mismatch: `{}` != `{}`",
                    node.name, correct_response.name
                );
            }

            NodeResult { failing: false }
        }
        Ok(None) => {
            warn!("Node `{}` is up but weird", node.name);

            NodeResult { failing: false }
        }
        Err(_) => NodeResult { failing: true },
    }
}

async fn call_obituary(
    client: &Client,
    me: &str,
    node: &NodeConfig,
    key: &str,
) -> Option<ObituaryResponse> {
    make_whatever_logged_http_call::<ObituaryResponse>(
        client,
        me,
        node,
        &format!("/obituary/{key}"),
        "obituary",
    )
    .await
    .ok()
    .flatten()
}

async fn call_silence_broadcast(
    client: &Client,
    me: &str,
    node: &NodeConfig,
    key: &str,
    silence: &NodeSilence,
) -> bool {
    info!(
        "Broadcasting silence {}: {}, to node `{}`",
        silence.id, silence.silent_until, node.name
    );
    let res = client
        .post(format!("{}/silence-broadcast/{key}", node.address))
        .json(&SilenceBroadcastRequest {
            id: silence.id,
            node_name: silence.node_name.clone(),
            silent_until: silence.silent_until,
        })
        .header(
            "User-Agent",
            format!("freecaster-grid/{}/{}", env!("CARGO_PKG_VERSION"), me,),
        )
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    let Ok(res) = res else {
        error!("Failed to connect to node {}: {:?}", node.name, res);
        return false;
    };

    res.status().is_success()
}

async fn announce_telegram(me: &str, target: &NodeConfig, config: &Arc<Config>, is_dead: bool) {
    let end = if let Some(tg) = target.telegram_handle.as_ref() {
        format!("- @{tg}")
    } else {
        "".to_string()
    };

    let res = telegram_notifyrs::send_message(
        if is_dead {
            format!(
                "Grid announcement, `{}` has unfortunately died, announced by: `{me}`{end}",
                target.name
            )
        } else {
            format!(
                "Grid announcement, `{}` has fortunately RETURNED, announced by: `{me}`{end}",
                target.name
            )
        },
        &config.telegram_token,
        config.telegram_chat_id,
    );
    if res.error() {
        error!("Telegram notification failed: {}", res.status());
    }
}
