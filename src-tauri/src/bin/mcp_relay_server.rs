//! Reference MCP cloud relay (development / self-hosted).
//!
//! Routes JSON-RPC between remote MCP clients and registered Ghost desktops.
//! Does not execute plans or persist file contents. Not a Ghost-hosted product
//! service — run this yourself or adapt the protocol in `docs/mcp-relay.md`.
//!
//! ```bash
//! GHOST_RELAY_SECRET=your-secret cargo run --bin mcp_relay_server --features experimental -- 8790
//! ```

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct RelayMessage {
    id: String,
    body: String,
}

#[derive(Default)]
struct DeviceState {
    inbox: Vec<RelayMessage>,
}

struct RelayState {
    devices: HashMap<String, DeviceState>,
    pending_responses: HashMap<String, String>,
}

fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(8790);
    let secret = std::env::var("GHOST_RELAY_SECRET").unwrap_or_else(|_| {
        eprintln!("Warning: GHOST_RELAY_SECRET not set — using dev-only default");
        "ghost-relay-dev-secret".into()
    });
    let state = Arc::new(Mutex::new(RelayState {
        devices: HashMap::new(),
        pending_responses: HashMap::new(),
    }));
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).expect("bind relay");
    eprintln!("Ghost MCP relay listening on http://{addr}");
    eprintln!("Protocol: docs/mcp-relay.md");

    for stream in listener.incoming().flatten() {
        let state = Arc::clone(&state);
        let secret = secret.clone();
        thread::spawn(move || {
            if let Err(e) = handle_client(stream, state, &secret) {
                eprintln!("relay client error: {e}");
            }
        });
    }
}

fn handle_client(
    mut stream: TcpStream,
    state: Arc<Mutex<RelayState>>,
    secret: &str,
) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(65)))
        .map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 65536];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let request = std::str::from_utf8(&buf[..n]).map_err(|e| e.to_string())?;
    let mut lines = request.lines();
    let request_line = lines.next().unwrap_or_default();
    let headers: Vec<&str> = lines.take_while(|l| !l.is_empty()).collect();
    let auth = headers
        .iter()
        .find_map(|h| h.strip_prefix("Authorization:").map(str::trim))
        .unwrap_or("");
    let token = auth.strip_prefix("Bearer ").unwrap_or("");
    if token != secret {
        return write_json(&mut stream, 401, r#"{"error":"unauthorized"}"#);
    }

    let body = extract_body(request, n);
    let path = request_line.split_whitespace().nth(1).unwrap_or("");

    match (request_line.split_whitespace().next(), path) {
        (Some("POST"), "/v1/device/register") => handle_register(&mut stream, state, body),
        (Some("GET"), p) if p.starts_with("/v1/device/poll") => handle_poll(&mut stream, state, p),
        (Some("POST"), "/v1/device/respond") => handle_respond(&mut stream, state, body),
        (Some("POST"), "/v1/mcp") => handle_mcp(&mut stream, state, body),
        _ => write_json(&mut stream, 404, r#"{"error":"not found"}"#),
    }
}

fn handle_register(
    stream: &mut TcpStream,
    state: Arc<Mutex<RelayState>>,
    body: &str,
) -> Result<(), String> {
    let parsed: serde_json::Value = serde_json::from_str(body).map_err(|_| "invalid json")?;
    let device_id = parsed
        .get("device_id")
        .and_then(|v| v.as_str())
        .ok_or("device_id required")?
        .to_string();
    let mut guard = state.lock().unwrap();
    guard.devices.entry(device_id).or_default();
    write_json(stream, 200, r#"{"ok":true}"#)
}

fn handle_poll(
    stream: &mut TcpStream,
    state: Arc<Mutex<RelayState>>,
    path: &str,
) -> Result<(), String> {
    let device_id = parse_query(path, "device_id").ok_or("device_id query required")?;
    let deadline = Instant::now() + Duration::from_secs(55);
    loop {
        {
            let mut guard = state.lock().unwrap();
            if let Some(device) = guard.devices.get_mut(&device_id) {
                if !device.inbox.is_empty() {
                    let messages: Vec<_> = device
                        .inbox
                        .drain(..)
                        .map(|m| serde_json::json!({"id": m.id, "body": m.body}))
                        .collect();
                    let text = serde_json::json!({ "messages": messages });
                    return write_json(stream, 200, &text.to_string());
                }
            }
        }
        if Instant::now() >= deadline {
            return write_json(stream, 200, r#"{"messages":[]}"#);
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn handle_respond(
    stream: &mut TcpStream,
    state: Arc<Mutex<RelayState>>,
    body: &str,
) -> Result<(), String> {
    let parsed: serde_json::Value = serde_json::from_str(body).map_err(|_| "invalid json")?;
    let id = parsed
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("id required")?
        .to_string();
    let response_body = parsed
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("{}")
        .to_string();
    state
        .lock()
        .unwrap()
        .pending_responses
        .insert(id, response_body);
    write_json(stream, 200, r#"{"ok":true}"#)
}

fn handle_mcp(
    stream: &mut TcpStream,
    state: Arc<Mutex<RelayState>>,
    body: &str,
) -> Result<(), String> {
    let parsed: serde_json::Value = serde_json::from_str(body).map_err(|_| "invalid json")?;
    let device_id = parsed
        .get("device_id")
        .and_then(|v| v.as_str())
        .ok_or("device_id required")?
        .to_string();
    let rpc_body = parsed
        .get("body")
        .and_then(|v| v.as_str())
        .ok_or("body required")?
        .to_string();
    let msg_id = uuid::Uuid::new_v4().to_string();
    {
        let mut guard = state.lock().unwrap();
        let device = guard.devices.entry(device_id).or_default();
        device.inbox.push(RelayMessage {
            id: msg_id.clone(),
            body: rpc_body,
        });
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(resp) = state.lock().unwrap().pending_responses.remove(&msg_id) {
            let text = serde_json::json!({ "body": resp });
            return write_json(stream, 200, &text.to_string());
        }
        if Instant::now() >= deadline {
            return write_json(stream, 504, r#"{"error":"device timeout"}"#);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn parse_query(path: &str, key: &str) -> Option<String> {
    let query = path.split('?').nth(1)?;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        if k == key {
            Some(v.to_string())
        } else {
            None
        }
    })
}

fn extract_body(request: &str, bytes_read: usize) -> &str {
    let content_length = request
        .lines()
        .filter_map(|line| line.strip_prefix("Content-Length:").map(str::trim))
        .next()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = request
        .find("\r\n\r\n")
        .map(|i| i + 4)
        .unwrap_or(bytes_read);
    let body = if body_start < bytes_read {
        &request[body_start..]
    } else {
        ""
    };
    if body.len() >= content_length {
        &body[..content_length.min(body.len())]
    } else {
        body
    }
}

fn write_json(stream: &mut TcpStream, status: u16, body: &str) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        504 => "Gateway Timeout",
        _ => "Not Found",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|e| e.to_string())
}
