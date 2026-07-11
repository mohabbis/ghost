//! OAuth 2.0 + PKCE sign-in for Microsoft (Entra ID) and Google, as public
//! clients (no client secret — PKCE is the whole point of not needing one on
//! a desktop app that can't keep a secret).
//!
//! This exists to answer "who is the user" for `commands/account.rs`, not to
//! move any workflow/organizer data off the device. It touches the network
//! (authorize/token/userinfo endpoints) and opens the system browser; nothing
//! else in Ghost calls out during this flow. See `docs/integrations-roadmap.md`
//! for how this identity is meant to be reused by future stack integrations
//! (Fabric/Power BI, Google Cloud, AI-assistant connectors).

use crate::accounts::AccountRecord;
use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::OsRng;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::Duration;

/// Loopback redirect must match what's registered with the identity
/// provider's app registration exactly, down to the trailing slash.
const REDIRECT_HOST: &str = "127.0.0.1";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Provider {
    Microsoft,
    Google,
}

impl Provider {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "microsoft" => Ok(Provider::Microsoft),
            "google" => Ok(Provider::Google),
            other => Err(format!(
                "Unknown sign-in provider '{other}' (expected 'microsoft' or 'google')"
            )),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Provider::Microsoft => "microsoft",
            Provider::Google => "google",
        }
    }

    fn authorize_endpoint(&self) -> &'static str {
        match self {
            Provider::Microsoft => "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            Provider::Google => "https://accounts.google.com/o/oauth2/v2/auth",
        }
    }

    fn token_endpoint(&self) -> &'static str {
        match self {
            Provider::Microsoft => "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            Provider::Google => "https://oauth2.googleapis.com/token",
        }
    }

    fn userinfo_endpoint(&self) -> &'static str {
        match self {
            Provider::Microsoft => "https://graph.microsoft.com/oidc/userinfo",
            Provider::Google => "https://openidconnect.googleapis.com/v1/userinfo",
        }
    }

    fn scopes(&self) -> &'static str {
        match self {
            // offline_access requests a refresh token; Ghost stores it so a
            // later integration doesn't force a fresh interactive sign-in.
            Provider::Microsoft => "openid email profile offline_access",
            Provider::Google => "openid email profile",
        }
    }

    /// Client ID to present to the provider. Config wins; the env var is a
    /// dev convenience for testing against a personal app registration
    /// without writing it to disk.
    pub fn client_id(&self, config: &crate::config::IntegrationSettings) -> Result<String, String> {
        let (configured, env_var) = match self {
            Provider::Microsoft => (&config.microsoft_client_id, "GHOST_MS_CLIENT_ID"),
            Provider::Google => (&config.google_client_id, "GHOST_GOOGLE_CLIENT_ID"),
        };
        if let Some(id) = configured {
            if !id.trim().is_empty() {
                return Ok(id.clone());
            }
        }
        if let Ok(id) = std::env::var(env_var) {
            if !id.trim().is_empty() {
                return Ok(id);
            }
        }
        Err(format!(
            "Sign in with {} isn't configured yet — set integrations.{}_client_id in Settings \
             (or {env_var} for local development) to a client ID from your own app registration.",
            match self {
                Provider::Microsoft => "Microsoft",
                Provider::Google => "Google",
            },
            self.name(),
        ))
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct MicrosoftUserInfo {
    email: Option<String>,
    name: Option<String>,
    #[serde(rename = "preferred_username")]
    preferred_username: Option<String>,
}

#[derive(Deserialize)]
struct GoogleUserInfo {
    email: Option<String>,
    name: Option<String>,
}

fn pkce_pair() -> (String, String) {
    let mut verifier_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut verifier_bytes);
    let verifier = B64URL.encode(verifier_bytes);
    let challenge = B64URL.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

fn random_state() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    B64URL.encode(bytes)
}

/// Bind a loopback listener on an OS-assigned port and wait for exactly one
/// browser redirect, returning its `code`/`state` query params. Blocks the
/// calling thread — callers must run this off the async runtime.
fn await_redirect(listener: TcpListener) -> anyhow::Result<(String, String)> {
    listener.set_nonblocking(false)?;
    let Some(stream) = listener.incoming().next() else {
        anyhow::bail!("Sign-in listener closed without receiving a callback");
    };
    let mut stream = stream?;
    stream.set_read_timeout(Some(CALLBACK_TIMEOUT)).ok();
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // "GET /callback?code=...&state=... HTTP/1.1"
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

/// Minimal percent-decoding — query values here are only ever an auth code
/// or an opaque state token, both ASCII, so a small manual decoder avoids
/// pulling in a URL-parsing crate for one call site.
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

/// Run the full interactive sign-in flow: open the system browser, wait for
/// the loopback redirect, exchange the code, fetch a minimal profile. Blocks
/// the calling thread for up to `CALLBACK_TIMEOUT`.
pub fn run_sign_in_flow(provider: Provider, client_id: &str) -> anyhow::Result<AccountRecord> {
    let listener = TcpListener::bind((REDIRECT_HOST, 0))?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://{REDIRECT_HOST}:{port}/callback");

    let (verifier, challenge) = pkce_pair();
    let state = random_state();
    let client_id_owned = client_id.to_string();

    let authorize_url = format!(
        "{}?client_id={}&response_type=code&redirect_uri={}&response_mode=query&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        provider.authorize_endpoint(),
        urlencoding_encode(&client_id_owned),
        urlencoding_encode(&redirect_uri),
        urlencoding_encode(provider.scopes()),
        urlencoding_encode(&state),
        urlencoding_encode(&challenge),
    );

    open::that(&authorize_url)
        .map_err(|e| anyhow::anyhow!("couldn't open the system browser for sign-in: {e}"))?;

    let (code, returned_state) = await_redirect(listener)?;
    if returned_state != state {
        anyhow::bail!("Sign-in state mismatch — the callback did not match the request Ghost sent (possible interception, aborting)");
    }

    let http = reqwest::blocking::Client::new();
    let token_res: TokenResponse = http
        .post(provider.token_endpoint())
        .form(&[
            ("client_id", client_id_owned.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("code_verifier", verifier.as_str()),
        ])
        .send()?
        .error_for_status()?
        .json()?;

    let profile_res = http
        .get(provider.userinfo_endpoint())
        .bearer_auth(&token_res.access_token)
        .send()?
        .error_for_status()?;

    let (email, name) = match provider {
        Provider::Microsoft => {
            let info: MicrosoftUserInfo = profile_res.json()?;
            let email = info.email.or(info.preferred_username).unwrap_or_default();
            (email, info.name.unwrap_or_default())
        }
        Provider::Google => {
            let info: GoogleUserInfo = profile_res.json()?;
            (
                info.email.unwrap_or_default(),
                info.name.unwrap_or_default(),
            )
        }
    };

    if email.is_empty() {
        anyhow::bail!(
            "{} did not return an account email",
            match provider {
                Provider::Microsoft => "Microsoft",
                Provider::Google => "Google",
            }
        );
    }

    Ok(AccountRecord {
        provider: provider.name().to_string(),
        email,
        name,
        refresh_token: token_res.refresh_token,
        linked_at: chrono::Utc::now(),
    })
}

fn urlencoding_encode(s: &str) -> String {
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
    fn provider_parse_round_trips_known_names() {
        assert_eq!(Provider::parse("microsoft").unwrap().name(), "microsoft");
        assert_eq!(Provider::parse("google").unwrap().name(), "google");
        assert!(Provider::parse("apple").is_err());
    }

    #[test]
    fn client_id_prefers_config_over_env() {
        std::env::set_var("GHOST_MS_CLIENT_ID", "env-id");
        let config = crate::config::IntegrationSettings {
            microsoft_client_id: Some("configured-id".to_string()),
            google_client_id: None,
        };
        assert_eq!(
            Provider::Microsoft.client_id(&config).unwrap(),
            "configured-id"
        );
        std::env::remove_var("GHOST_MS_CLIENT_ID");
    }

    #[test]
    fn client_id_falls_back_to_env_var() {
        std::env::set_var("GHOST_GOOGLE_CLIENT_ID", "env-google-id");
        let config = crate::config::IntegrationSettings::default();
        assert_eq!(
            Provider::Google.client_id(&config).unwrap(),
            "env-google-id"
        );
        std::env::remove_var("GHOST_GOOGLE_CLIENT_ID");
    }

    #[test]
    fn client_id_errors_when_unconfigured() {
        std::env::remove_var("GHOST_MS_CLIENT_ID");
        let config = crate::config::IntegrationSettings::default();
        let err = Provider::Microsoft.client_id(&config).unwrap_err();
        assert!(err.contains("Microsoft"));
    }

    #[test]
    fn pkce_challenge_is_sha256_of_verifier() {
        let (verifier, challenge) = pkce_pair();
        let expected = B64URL.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, expected);
        assert_ne!(verifier, challenge);
    }

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
