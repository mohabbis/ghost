//! Outbound MCP cloud relay client (experimental).
//!
//! The desktop opens an outbound HTTPS connection to a user-configured relay so
//! wide-area MCP clients can reach Ghost without inbound firewall rules. The
//! relay routes JSON-RPC only — it never executes plans or reads file contents.
//! See `docs/mcp-relay.md`.

use super::server::handle_message;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelayOptions {
    pub relay_url: String,
    pub device_token: String,
    pub device_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct RelayConfigStore {
    relay_url: Option<String>,
    device_id: Option<String>,
    device_token_hint: Option<String>,
}

static RUNNING: AtomicBool = AtomicBool::new(false);

struct RelayHandle {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    options: RelayOptions,
}

static RELAY: Mutex<Option<RelayHandle>> = Mutex::new(None);

fn store_path() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ghost")
        .join("mcp-relay.json")
}

fn persist_options(options: &RelayOptions) {
    let store = RelayConfigStore {
        relay_url: Some(options.relay_url.clone()),
        device_id: Some(options.device_id.clone()),
        device_token_hint: token_hint(&options.device_token),
    };
    if let Some(parent) = store_path().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_vec(&store) {
        let _ = std::fs::write(store_path(), json);
    }
}

fn token_hint(token: &str) -> Option<String> {
    if token.len() <= 4 {
        Some("****".into())
    } else {
        Some(format!("{}…", &token[..4]))
    }
}

pub fn saved_relay_url() -> Option<String> {
    let bytes = std::fs::read(store_path()).ok()?;
    serde_json::from_slice::<RelayConfigStore>(&bytes)
        .ok()
        .and_then(|s| s.relay_url)
}

#[derive(Clone, Debug, Serialize)]
pub struct RelayStatus {
    pub connected: bool,
    pub relay_url: Option<String>,
    pub device_id: Option<String>,
    pub token_hint: Option<String>,
}

pub fn relay_status() -> RelayStatus {
    let saved: RelayConfigStore = std::fs::read(store_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let guard = RELAY.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(handle) = guard.as_ref() {
        RelayStatus {
            connected: RUNNING.load(Ordering::SeqCst),
            relay_url: Some(handle.options.relay_url.clone()),
            device_id: Some(handle.options.device_id.clone()),
            token_hint: token_hint(&handle.options.device_token),
        }
    } else {
        RelayStatus {
            connected: false,
            relay_url: saved.relay_url,
            device_id: saved.device_id,
            token_hint: saved.device_token_hint,
        }
    }
}

pub fn start_relay(options: RelayOptions) -> Result<(), String> {
    let relay_url = options.relay_url.trim().trim_end_matches('/').to_string();
    if relay_url.is_empty() {
        return Err("relay URL is required".into());
    }
    if !relay_url.starts_with("https://") {
        return Err("relay URL must use HTTPS (https://)".into());
    }
    if options.device_id.trim().is_empty() {
        return Err("device_id is required".into());
    }
    if options.device_token.trim().is_empty() {
        return Err("device token is required".into());
    }
    let mut opts = options;
    opts.relay_url = relay_url;
    persist_options(&opts);
    stop_relay();

    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);
    let join_opts = opts.clone();
    let join = thread::spawn(move || relay_loop(join_opts, stop_thread));

    let mut guard = RELAY.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(RelayHandle {
        stop,
        join: Some(join),
        options: opts,
    });
    Ok(())
}

pub fn stop_relay() {
    let mut guard = RELAY.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut handle) = guard.take() {
        handle.stop.store(true, Ordering::SeqCst);
        if let Some(join) = handle.join.take() {
            let _ = join.join();
        }
    }
    RUNNING.store(false, Ordering::SeqCst);
}

#[derive(Deserialize)]
struct PollResponse {
    messages: Vec<RelayMessage>,
}

#[derive(Deserialize)]
struct RelayMessage {
    id: String,
    body: String,
}

fn relay_loop(options: RelayOptions, stop: Arc<AtomicBool>) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());

    let register_url = format!("{}/v1/device/register", options.relay_url);
    let poll_url = format!("{}/v1/device/poll", options.relay_url);
    let respond_url = format!("{}/v1/device/respond", options.relay_url);

    let reg_body = serde_json::json!({
        "device_id": options.device_id,
    });
    if let Err(e) = client
        .post(&register_url)
        .bearer_auth(&options.device_token)
        .json(&reg_body)
        .send()
    {
        eprintln!("MCP relay register failed: {e}");
        return;
    }

    RUNNING.store(true, Ordering::SeqCst);
    eprintln!(
        "MCP relay connected to {} as device {}",
        options.relay_url, options.device_id
    );

    while !stop.load(Ordering::SeqCst) {
        let poll = client
            .get(&poll_url)
            .bearer_auth(&options.device_token)
            .query(&[("device_id", options.device_id.as_str())])
            .send();

        match poll {
            Ok(resp) if resp.status().is_success() => {
                let Ok(body) = resp.json::<PollResponse>() else {
                    thread::sleep(Duration::from_millis(500));
                    continue;
                };
                for msg in body.messages {
                    let response = handle_message(&msg.body);
                    let text = serde_json::to_string(&response).unwrap_or_else(|_| "{}".into());
                    let respond_body = serde_json::json!({
                        "id": msg.id,
                        "body": text,
                    });
                    let _ = client
                        .post(&respond_url)
                        .bearer_auth(&options.device_token)
                        .json(&respond_body)
                        .send();
                }
            }
            Ok(_) => thread::sleep(Duration::from_secs(2)),
            Err(e) if e.is_timeout() => {}
            Err(e) => {
                eprintln!("MCP relay poll error: {e}");
                thread::sleep(Duration::from_secs(5));
            }
        }
    }
    RUNNING.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_url_must_be_https() {
        let err = start_relay(RelayOptions {
            relay_url: "http://relay.example".into(),
            device_token: "tok".into(),
            device_id: "dev".into(),
        })
        .unwrap_err();
        assert!(err.contains("HTTPS"));
    }
}
