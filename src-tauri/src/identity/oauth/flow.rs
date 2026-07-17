//! Interactive OAuth sign-in flow: browser, loopback callback, token exchange.

use crate::identity::types::{AccountIdentity, TokenMaterial};
use serde::Deserialize;
use std::net::TcpListener;

use super::callback::{CALLBACK_TIMEOUT, await_redirect, urlencoding_encode};
use super::pkce::{constant_time_eq, pkce_pair, random_state};
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

/// Result of an incremental-consent grant flow: tokens only — the caller
/// already has an `AccountIdentity` from a prior sign-in, so there's no
/// profile to (re)fetch.
pub struct GrantResult {
    pub tokens: TokenMaterial,
}

/// Tokens from a completed PKCE + loopback authorization, plus the HTTP
/// client used to obtain them (reused by callers that need a follow-up
/// request, e.g. the userinfo fetch during sign-in).
struct AuthorizedTokens {
    http: reqwest::blocking::Client,
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Assemble the provider consent-screen URL for an authorization-code + PKCE
/// flow. Pulled out of `authorize_and_exchange` so the security-relevant query
/// assembly — PKCE `S256` challenge binding and the CSRF `state` — can be
/// asserted without opening a browser or binding a socket. Every dynamic value
/// is percent-encoded through `urlencoding_encode`.
fn build_authorize_url(
    provider: OAuthProvider,
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
    challenge: &str,
) -> String {
    format!(
        "{}?client_id={}&response_type=code&redirect_uri={}&response_mode=query&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        provider.authorize_endpoint(),
        urlencoding_encode(client_id),
        urlencoding_encode(redirect_uri),
        urlencoding_encode(scope),
        urlencoding_encode(state),
        urlencoding_encode(challenge),
    )
}

/// Shared core of every OAuth flow Ghost runs: bind a loopback listener, open
/// the system browser to `provider`'s consent screen for `scope`, wait for
/// the redirect, and exchange the code for tokens. Blocks up to
/// `CALLBACK_TIMEOUT`. Used by both sign-in (base identity scopes) and
/// integration-grant requests (e.g. Power BI's scope) — the two differ only
/// in which scope string they request and what they do with the resulting
/// access token afterward.
fn authorize_and_exchange(
    provider: OAuthProvider,
    client_id: &str,
    scope: &str,
) -> anyhow::Result<AuthorizedTokens> {
    let listener = TcpListener::bind((REDIRECT_HOST, 0))?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://{REDIRECT_HOST}:{port}/callback");

    let (verifier, challenge) = pkce_pair();
    let state = random_state();
    let client_id_owned = client_id.to_string();

    let authorize_url = build_authorize_url(
        provider,
        &client_id_owned,
        &redirect_uri,
        scope,
        &state,
        &challenge,
    );

    open::that(&authorize_url)
        .map_err(|e| anyhow::anyhow!("couldn't open the system browser for sign-in: {e}"))?;

    let (code, returned_state) = await_redirect(listener)?;
    if !constant_time_eq(&returned_state, &state) {
        anyhow::bail!(
            "Sign-in state mismatch — the callback did not match the request Ghost sent (possible interception, aborting)"
        );
    }

    let http = reqwest::blocking::Client::builder()
        .timeout(CALLBACK_TIMEOUT)
        .build()?;
    // reqwest 0.13's blocking RequestBuilder has no `.form()`; build the
    // x-www-form-urlencoded body explicitly.
    let token_body = form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", client_id_owned.as_str())
        .append_pair("grant_type", "authorization_code")
        .append_pair("code", code.as_str())
        .append_pair("redirect_uri", redirect_uri.as_str())
        .append_pair("code_verifier", verifier.as_str())
        .finish();
    let token_res: TokenResponse = http
        .post(provider.token_endpoint())
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(token_body)
        .send()?
        .error_for_status()?
        .json()?;

    let expires_at = token_res
        .expires_in
        .map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs as i64));

    Ok(AuthorizedTokens {
        http,
        access_token: token_res.access_token,
        refresh_token: token_res.refresh_token,
        expires_at,
    })
}

/// Run the full interactive sign-in flow. Blocks up to `CALLBACK_TIMEOUT`.
pub fn run_sign_in_flow(provider: OAuthProvider, client_id: &str) -> anyhow::Result<SignInResult> {
    let authorized = authorize_and_exchange(provider, client_id, provider.identity_scopes())?;

    let profile_res = authorized
        .http
        .get(provider.userinfo_endpoint())
        .bearer_auth(&authorized.access_token)
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
            access_token: authorized.access_token,
            refresh_token: authorized.refresh_token,
            expires_at: authorized.expires_at,
        },
        scopes: provider.identity_scope_list(),
    })
}

/// Run the PKCE + loopback consent flow for an additional scope (e.g. Power
/// BI), reusing the same browser/listener/token-exchange plumbing as sign-in.
/// Does not touch `AccountIdentity` or fetch a profile — the caller already
/// has an identity from a prior `run_sign_in_flow` call.
pub fn run_grant_flow(
    provider: OAuthProvider,
    client_id: &str,
    scope: &str,
) -> anyhow::Result<GrantResult> {
    let authorized = authorize_and_exchange(provider, client_id, scope)?;
    Ok(GrantResult {
        tokens: TokenMaterial {
            access_token: authorized.access_token,
            refresh_token: authorized.refresh_token,
            expires_at: authorized.expires_at,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_query(url: &str) -> std::collections::HashMap<String, String> {
        let query = url.split_once('?').map(|x| x.1).unwrap_or("");
        query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn authorize_url_targets_the_provider_consent_endpoint() {
        let url = build_authorize_url(
            OAuthProvider::Microsoft,
            "client-abc",
            "http://127.0.0.1:5555/callback",
            "openid email",
            "state-xyz",
            "challenge-123",
        );
        assert!(url.starts_with(OAuthProvider::Microsoft.authorize_endpoint()));
        assert!(url.contains('?'));
    }

    #[test]
    fn authorize_url_requests_a_code_via_pkce_s256() {
        let url = build_authorize_url(
            OAuthProvider::Google,
            "client-abc",
            "http://127.0.0.1:5555/callback",
            "openid email",
            "state-xyz",
            "challenge-123",
        );
        let params = parse_query(&url);
        assert_eq!(
            params.get("response_type").map(String::as_str),
            Some("code")
        );
        // The PKCE challenge must be present and use SHA-256, never the
        // "plain" method — that is the whole point of the code-exchange guard.
        assert_eq!(
            params.get("code_challenge").map(String::as_str),
            Some("challenge-123")
        );
        assert_eq!(
            params.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
    }

    #[test]
    fn authorize_url_carries_state_client_and_scope() {
        let url = build_authorize_url(
            OAuthProvider::Google,
            "client-abc",
            "http://127.0.0.1:5555/callback",
            "openid email",
            "state-xyz",
            "challenge-123",
        );
        let params = parse_query(&url);
        // CSRF state round-trips verbatim (await_redirect compares it back).
        assert_eq!(params.get("state").map(String::as_str), Some("state-xyz"));
        assert_eq!(
            params.get("client_id").map(String::as_str),
            Some("client-abc")
        );
        // Reserved characters in the redirect and scope are percent-encoded,
        // never emitted raw into the query string.
        assert_eq!(
            params.get("redirect_uri").map(String::as_str),
            Some("http%3A%2F%2F127.0.0.1%3A5555%2Fcallback")
        );
        assert_eq!(
            params.get("scope").map(String::as_str),
            Some("openid%20email")
        );
    }
}
