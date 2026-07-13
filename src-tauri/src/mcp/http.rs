//! Localhost-only HTTP transport for MCP (experimental remote access v0).
//!
//! Binds to 127.0.0.1 only. Reuses the same JSON-RPC handler as stdio MCP.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::server::handle_message;

static RUNNING: AtomicBool = AtomicBool::new(false);

/// Start a blocking localhost HTTP server on `port` until `stop` is set.
pub fn run_localhost_http(port: u16, stop: Arc<AtomicBool>) {
    let addr = format!("127.0.0.1:{port}");
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("MCP HTTP bind failed on {addr}: {e}");
            return;
        }
    };
    RUNNING.store(true, Ordering::SeqCst);
    listener.set_nonblocking(true).expect("set_nonblocking");
    eprintln!("Ghost MCP HTTP listening on http://{addr}/mcp (localhost only)");

    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                thread::spawn(move || {
                    if let Err(err) = handle_http_connection(stream) {
                        eprintln!("MCP HTTP connection error: {err}");
                    }
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => eprintln!("MCP HTTP accept error: {e}"),
        }
    }
    RUNNING.store(false, Ordering::SeqCst);
}

pub fn localhost_http_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

fn handle_http_connection(mut stream: TcpStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let request = std::str::from_utf8(&buf[..n]).map_err(|e| e.to_string())?;
    let mut lines = request.lines();
    let request_line = lines.next().unwrap_or_default();
    if !request_line.starts_with("POST /mcp") {
        return write_response(&mut stream, 404, r#"{"error":"not found"}"#);
    }
    let content_length = lines
        .filter_map(|line| line.strip_prefix("Content-Length:").map(str::trim))
        .next()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(n);
    let body = if body_start < n {
        &request[body_start..]
    } else {
        ""
    };
    let body = if body.len() >= content_length {
        &body[..content_length.min(body.len())]
    } else {
        body
    };
    let response = handle_message(body.trim());
    let text = serde_json::to_string(&response).unwrap_or_else(|_| "{}".into());
    write_response(&mut stream, 200, &text)
}

fn write_response(stream: &mut TcpStream, status: u16, body: &str) -> Result<(), String> {
    let reason = if status == 200 { "OK" } else { "Not Found" };
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
}
