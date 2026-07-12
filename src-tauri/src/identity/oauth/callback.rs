use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::Duration;

pub const CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);

/// Bind a loopback listener and wait for one browser redirect with `code`/`state`.
pub fn await_redirect(listener: TcpListener) -> anyhow::Result<(String, String)> {
    listener.set_nonblocking(false)?;
    let Some(stream) = listener.incoming().next() else {
        anyhow::bail!("Sign-in listener closed without receiving a callback");
    };
    let mut stream = stream?;
    stream.set_read_timeout(Some(CALLBACK_TIMEOUT)).ok();
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .to_string();
    let query = path.split_once('?').map(|x| x.1).unwrap_or("").to_string();

    let body =
        "<html><body><p>Signed in. You can close this tab and return to Ghost.</p></body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("code"), Some(v)) => code = Some(urlencoding_decode(v)),
            (Some("state"), Some(v)) => state = Some(urlencoding_decode(v)),
            _ => {}
        }
    }

    match (code, state) {
        (Some(c), Some(s)) => Ok((c, s)),
        _ => anyhow::bail!("Sign-in was cancelled or the provider returned no authorization code"),
    }
}

fn urlencoding_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoding_round_trips_reserved_characters() {
        let encoded = urlencoding_encode("a b+c/d=e&f");
        assert_eq!(urlencoding_decode(&encoded), "a b+c/d=e&f");
    }

    #[test]
    fn urlencoding_decode_handles_plus_as_space() {
        assert_eq!(urlencoding_decode("a+b"), "a b");
    }
}
