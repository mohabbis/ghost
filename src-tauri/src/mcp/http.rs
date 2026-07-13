//! HTTP transport for MCP and inbound Fabric webhooks (experimental remote access).
//!
//! Default bind is loopback (`127.0.0.1`). LAN exposure (`0.0.0.0`) requires a
//! bearer token. Optional in-process TLS loads PEM cert/key paths (experimental).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::server::handle_message;

static RUNNING: AtomicBool = AtomicBool::new(false);

/// Options for the combined MCP + Fabric webhook HTTP listener.
#[derive(Clone, Debug)]
pub struct HttpServerOptions {
    pub bind_host: String,
    pub port: u16,
    /// Required when `bind_host` is not loopback.
    pub bearer_token: Option<String>,
    pub fabric_webhook_secret: Option<String>,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
}

impl Default for HttpServerOptions {
    fn default() -> Self {
        Self {
            bind_host: "127.0.0.1".to_string(),
            port: 8787,
            bearer_token: None,
            fabric_webhook_secret: None,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

struct ServerHandle {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    options: HttpServerOptions,
}

static SERVER: Mutex<Option<ServerHandle>> = Mutex::new(None);

fn is_loopback(host: &str) -> bool {
    host == "127.0.0.1" || host == "localhost" || host == "::1"
}

/// Start (or restart) the HTTP server with the given options.
pub fn start_http_server(options: HttpServerOptions) -> Result<(), String> {
    if !is_loopback(&options.bind_host) && options.bearer_token.as_deref().unwrap_or("").is_empty()
    {
        return Err(
            "LAN exposure requires a bearer token — set one before binding beyond localhost".into(),
        );
    }
    #[cfg(feature = "experimental")]
    let tls_config = match (
        options.tls_cert_path.as_deref(),
        options.tls_key_path.as_deref(),
    ) {
        (Some(cert), Some(key)) => Some(super::tls::load_server_config(
            std::path::Path::new(cert),
            std::path::Path::new(key),
        )?),
        (None, None) => None,
        _ => {
            return Err("TLS requires both tls_cert_path and tls_key_path".into());
        }
    };
    stop_http_server();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);
    let opts = options.clone();
    #[cfg(feature = "experimental")]
    let tls_for_thread = tls_config.clone();
    let join = thread::spawn(move || {
        #[cfg(feature = "experimental")]
        {
            run_http_listener(opts, stop_thread, tls_for_thread);
        }
        #[cfg(not(feature = "experimental"))]
        {
            run_http_listener(opts, stop_thread);
        }
    });
    let mut guard = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(ServerHandle {
        stop,
        join: Some(join),
        options,
    });
    Ok(())
}

/// Stop the HTTP server if running.
pub fn stop_http_server() {
    let mut guard = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut handle) = guard.take() {
        handle.stop.store(true, Ordering::SeqCst);
        if let Some(join) = handle.join.take() {
            let _ = join.join();
        }
    }
    RUNNING.store(false, Ordering::SeqCst);
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct HttpServerStatus {
    pub running: bool,
    pub bind_host: String,
    pub port: u16,
    pub lan_exposed: bool,
    pub bearer_required: bool,
    pub fabric_webhook_enabled: bool,
    pub tls_enabled: bool,
}

pub fn http_server_status() -> HttpServerStatus {
    let guard = SERVER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(handle) = guard.as_ref() {
        let lan = !is_loopback(&handle.options.bind_host);
        HttpServerStatus {
            running: RUNNING.load(Ordering::SeqCst),
            bind_host: handle.options.bind_host.clone(),
            port: handle.options.port,
            lan_exposed: lan,
            bearer_required: lan || handle.options.bearer_token.is_some(),
            fabric_webhook_enabled: handle
                .options
                .fabric_webhook_secret
                .as_ref()
                .is_some_and(|s| !s.is_empty()),
            tls_enabled: handle.options.tls_cert_path.is_some(),
        }
    } else {
        HttpServerStatus {
            running: false,
            bind_host: "127.0.0.1".to_string(),
            port: 8787,
            lan_exposed: false,
            bearer_required: false,
            fabric_webhook_enabled: false,
            tls_enabled: false,
        }
    }
}

/// Blocking localhost-only HTTP server (CLI `ghost mcp serve http`).
pub fn run_localhost_http(port: u16, stop: Arc<AtomicBool>) {
    #[cfg(feature = "experimental")]
    run_http_listener(
        HttpServerOptions {
            bind_host: "127.0.0.1".to_string(),
            port,
            ..Default::default()
        },
        stop,
        None,
    );
    #[cfg(not(feature = "experimental"))]
    run_http_listener(
        HttpServerOptions {
            bind_host: "127.0.0.1".to_string(),
            port,
            ..Default::default()
        },
        stop,
    );
}

pub fn localhost_http_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

#[cfg(feature = "experimental")]
fn run_http_listener(
    options: HttpServerOptions,
    stop: Arc<AtomicBool>,
    tls_config: Option<Arc<rustls::ServerConfig>>,
) {
    run_http_listener_impl(options, stop, tls_config);
}

#[cfg(not(feature = "experimental"))]
fn run_http_listener(options: HttpServerOptions, stop: Arc<AtomicBool>) {
    run_http_listener_impl(options, stop, None);
}

fn run_http_listener_impl(
    options: HttpServerOptions,
    stop: Arc<AtomicBool>,
    #[cfg(feature = "experimental")] tls_config: Option<Arc<rustls::ServerConfig>>,
    #[cfg(not(feature = "experimental"))] _tls_config: Option<()>,
) {
    let addr = format!("{}:{}", options.bind_host, options.port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("HTTP bind failed on {addr}: {e}");
            return;
        }
    };
    RUNNING.store(true, Ordering::SeqCst);
    listener.set_nonblocking(true).expect("set_nonblocking");
    let scope = if is_loopback(&options.bind_host) {
        "localhost"
    } else {
        "LAN"
    };
    let scheme = {
        #[cfg(feature = "experimental")]
        {
            if tls_config.is_some() {
                "https"
            } else {
                "http"
            }
        }
        #[cfg(not(feature = "experimental"))]
        {
            "http"
        }
    };
    eprintln!(
        "Ghost HTTP listening on {scheme}://{addr} ({scope}) — POST /mcp, POST /fabric/webhook"
    );

    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let opts = options.clone();
                #[cfg(feature = "experimental")]
                {
                    let tls = tls_config.clone();
                    thread::spawn(move || {
                        if let Err(err) = serve_connection(stream, &opts, tls) {
                            eprintln!("HTTP connection error: {err}");
                        }
                    });
                }
                #[cfg(not(feature = "experimental"))]
                {
                    thread::spawn(move || {
                        if let Err(err) = serve_connection(stream, &opts) {
                            eprintln!("HTTP connection error: {err}");
                        }
                    });
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => eprintln!("HTTP accept error: {e}"),
        }
    }
    RUNNING.store(false, Ordering::SeqCst);
}

#[cfg(feature = "experimental")]
fn serve_connection(
    mut stream: TcpStream,
    options: &HttpServerOptions,
    tls_config: Option<Arc<rustls::ServerConfig>>,
) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;
    if let Some(cfg) = tls_config {
        let mut conn = rustls::ServerConnection::new(cfg).map_err(|e| e.to_string())?;
        conn.complete_io(&mut stream)
            .map_err(|e| format!("TLS handshake failed: {e}"))?;
        let mut tls = rustls::Stream::new(&mut conn, &mut stream);
        handle_connection_io(&mut tls, options)
    } else {
        handle_connection_io(&mut stream, options)
    }
}

#[cfg(not(feature = "experimental"))]
fn serve_connection(mut stream: TcpStream, options: &HttpServerOptions) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;
    handle_connection_io(&mut stream, options)
}

fn handle_connection_io<S: Read + Write>(
    stream: &mut S,
    options: &HttpServerOptions,
) -> Result<(), String> {
    let mut buf = vec![0u8; 16384];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let request = std::str::from_utf8(&buf[..n]).map_err(|e| e.to_string())?;
    let mut lines = request.lines();
    let request_line = lines.next().unwrap_or_default();
    let headers: Vec<&str> = lines.take_while(|l| !l.is_empty()).collect();

    if request_line.starts_with("POST /mcp") {
        if let Err(reason) = check_bearer(&headers, options) {
            return write_response(stream, 401, &format!(r#"{{"error":"{reason}"}}"#));
        }
        let body = extract_body(request, n);
        let response = handle_message(body.trim());
        let text = serde_json::to_string(&response).unwrap_or_else(|_| "{}".into());
        return write_response(stream, 200, &text);
    }

    if request_line.starts_with("POST /fabric/webhook") {
        if let Err(reason) = check_webhook_secret(&headers, options) {
            return write_response(stream, 401, &format!(r#"{{"error":"{reason}"}}"#));
        }
        let body = extract_body(request, n);
        return handle_fabric_webhook(stream, body);
    }

    write_response(stream, 404, r#"{"error":"not found"}"#)
}

fn check_bearer(headers: &[&str], options: &HttpServerOptions) -> Result<(), &'static str> {
    let require = !is_loopback(&options.bind_host) || options.bearer_token.is_some();
    if !require {
        return Ok(());
    }
    let expected = options.bearer_token.as_deref().unwrap_or("");
    if expected.is_empty() {
        return Err("bearer token required");
    }
    let auth = headers
        .iter()
        .find_map(|h| h.strip_prefix("Authorization:").map(str::trim))
        .unwrap_or("");
    let token = auth.strip_prefix("Bearer ").unwrap_or("");
    if token == expected {
        Ok(())
    } else {
        Err("invalid bearer token")
    }
}

fn check_webhook_secret(headers: &[&str], options: &HttpServerOptions) -> Result<(), &'static str> {
    let expected = options
        .fabric_webhook_secret
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or("webhook secret not configured")?;
    let provided = headers
        .iter()
        .find_map(|h| h.strip_prefix("X-Ghost-Webhook-Secret:").map(str::trim))
        .unwrap_or("");
    if provided == expected {
        Ok(())
    } else {
        Err("invalid webhook secret")
    }
}

fn handle_fabric_webhook<S: Write>(stream: &mut S, body: &str) -> Result<(), String> {
    let parsed = crate::integrations::microsoft::fabric::inbound_payload::parse_inbound_body(body)?;
    let intent = crate::integrations::microsoft::fabric::triggers::record_intent(
        parsed.zone_id,
        &parsed.source,
        &parsed.summary,
    );
    let text = serde_json::to_string(&intent).unwrap_or_else(|_| "{}".into());
    write_response(stream, 200, &text)
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

fn write_response<S: Write>(stream: &mut S, status: u16, body: &str) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localhost_flag_defaults_false() {
        assert!(!localhost_http_running());
    }

    #[test]
    fn lan_bind_requires_bearer() {
        let err = start_http_server(HttpServerOptions {
            bind_host: "0.0.0.0".to_string(),
            port: 0,
            bearer_token: None,
            fabric_webhook_secret: None,
            tls_cert_path: None,
            tls_key_path: None,
        })
        .unwrap_err();
        assert!(err.contains("bearer"));
    }

    #[test]
    fn bearer_check_accepts_matching_token() {
        let opts = HttpServerOptions {
            bearer_token: Some("secret".into()),
            ..Default::default()
        };
        let headers = ["Authorization: Bearer secret"];
        assert!(check_bearer(&headers, &opts).is_ok());
    }
}
