use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

pub const CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);

/// How long to sleep between non-blocking accept attempts while waiting for
/// the browser to redirect back.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Bind a loopback listener and wait for a browser redirect carrying
/// `code`/`state`.
///
/// Browsers routinely open incidental connections to whatever origin they're
/// displaying (a `/favicon.ico` probe is the common case) — accepting only
/// the first connection and treating it as the callback would let one of
/// those steal the real redirect and report sign-in as cancelled. So this
/// skips any connection that doesn't parse into a `code`+`state` pair and
/// keeps waiting. It also enforces one overall deadline across every accept
/// attempt: if the user closes the tab and no redirect ever arrives, this
/// still returns (with an error) after `CALLBACK_TIMEOUT` instead of blocking
/// the caller forever.
pub fn await_redirect(listener: TcpListener) -> anyhow::Result<(String, String)> {
    await_redirect_with_timeout(listener, CALLBACK_TIMEOUT)
}

fn await_redirect_with_timeout(
    listener: TcpListener,
    timeout: Duration,
) -> anyhow::Result<(String, String)> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + timeout;

    loop {
        if Instant::now() >= deadline {
            anyhow::bail!("Sign-in timed out waiting for the browser to complete the redirect");
        }

        let mut stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(POLL_INTERVAL);
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(timeout)).ok();

        let mut request_line = String::new();
        {
            let mut reader = BufReader::new(stream.try_clone()?);
            if reader.read_line(&mut request_line).is_err() {
                continue; // malformed/aborted connection; keep waiting for the real one
            }
        }

        let path = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .to_string();
        let query = path.split_once('?').map(|x| x.1).unwrap_or("").to_string();

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

        let (Some(code), Some(state)) = (code, state) else {
            // Not the OAuth redirect — respond plainly and keep waiting
            // rather than treating this stray connection as a cancelled
            // sign-in.
            let _ = respond(&mut stream, "404 Not Found", "");
            continue;
        };

        respond(
            &mut stream,
            "200 OK",
            "<html><body><p>Signed in. You can close this tab and return to Ghost.</p></body></html>",
        )?;
        return Ok((code, state));
    }
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) -> anyhow::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
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

    #[test]
    fn stray_connection_is_skipped_and_the_real_callback_is_still_found() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();

        std::thread::spawn(move || {
            // A browser's incidental favicon probe — no code/state at all.
            let mut favicon = TcpStream::connect(addr).unwrap();
            favicon
                .write_all(b"GET /favicon.ico HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
                .unwrap();
            drop(favicon);

            std::thread::sleep(Duration::from_millis(50));

            // The real OAuth redirect, arriving second.
            let mut real = TcpStream::connect(addr).unwrap();
            real.write_all(
                b"GET /callback?code=abc123&state=xyz789 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
            )
            .unwrap();
        });

        let (code, state) = await_redirect_with_timeout(listener, Duration::from_secs(5)).unwrap();
        assert_eq!(code, "abc123");
        assert_eq!(state, "xyz789");
    }

    #[test]
    fn times_out_instead_of_blocking_forever_when_nothing_arrives() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let err = await_redirect_with_timeout(listener, Duration::from_millis(200))
            .expect_err("should time out, not hang");
        assert!(err.to_string().contains("timed out"));
    }
}
