//! Interactive OAuth sign-in flow: browser, loopback callback, token exchange.

use crate::identity::types::{AccountIdentity, TokenMaterial};
use serde::Deserialize;
use std::net::TcpListener;

use super::callback::{await_redirect, urlencoding_encode, CALLBACK_TIMEOUT};
use super::pkce::{pkce_pair, random_state};
use super::provider::{OAuthProvider, REDIRECT_HOST};

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

#[derive(Deserialize)]
struct MicrosoftUserInfo {
    email: Option<String>,
    name: Option<String>,
    #[serde(rename = "preferred_username")]
    preferred_username: Option<String>,
    sub: Option<String>,
    tid: Option<String>,
}

#[derive(Deserialize)]
struct GoogleUserInfo {
    email: Option<String>,
    name: Option<String>,
    sub: Option<String>,
}

pub struct SignInResult {
    pub identity: AccountIdentity,
    pub tokens: TokenMaterial,
    pub scopes: Vec<&'static str>,
}

/// Run the full interactive sign-in flow. Blocks up to `CALLBACK_TIMEOUT`.
pub fn run_sign_in_flow(provider: OAuthProvider, client_id: &str) -> anyhow::Result<SignInResult> {
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
        urlencoding_encode(provider.identity_scopes()),
        urlencoding_encode(&state),
        urlencoding_encode(&challenge),
    );

    open::that(&authorize_url)
        .map_err(|e| anyhow::anyhow!("couldn't open the system browser for sign-in: {e}"))?;

    let (code, returned_state) = await_redirect(listener)?;
    if returned_state != state {
        anyhow::bail!(
            "Sign-in state mismatch — the callback did not match the request Ghost sent (possible interception, aborting)"
        );
    }

    let http = reqwest::blocking::Client::builder()
        .timeout(CALLBACK_TIMEOUT)
        .build()?;
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

    let expires_at = token_res
        .expires_in
        .map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs as i64));

    let profile_res = http
        .get(provider.userinfo_endpoint())
        .bearer_auth(&token_res.access_token)
        .send()?
        .error_for_status()?;

    let linked_at = chrono::Utc::now();
    let account_id = uuid::Uuid::new_v4().to_string();

    let (email, display_name, subject, tenant_id) = match provider {
        OAuthProvider::Microsoft => {
            let info: MicrosoftUserInfo = profile_res.json()?;
            let email = info.email.or(info.preferred_username).unwrap_or_default();
            let subject = info.sub.unwrap_or_else(|| email.clone());
            (email, info.name.unwrap_or_default(), subject, info.tid)
        }
        OAuthProvider::Google => {
            let info: GoogleUserInfo = profile_res.json()?;
            let email = info.email.unwrap_or_default();
            let subject = info.sub.unwrap_or_else(|| email.clone());
            (email, info.name.unwrap_or_default(), subject, None)
        }
    };

    if email.is_empty() {
        anyhow::bail!(
            "{} did not return an account email",
            match provider {
                OAuthProvider::Microsoft => "Microsoft",
                OAuthProvider::Google => "Google",
            }
        );
    }

    Ok(SignInResult {
        identity: AccountIdentity {
            account_id,
            provider: provider.identity_provider(),
            subject,
            tenant_id,
            email,
            display_name,
            linked_at,
        },
        tokens: TokenMaterial {
            access_token: token_res.access_token,
            refresh_token: token_res.refresh_token,
            expires_at,
        },
        scopes: provider.identity_scope_list(),
    })
}
