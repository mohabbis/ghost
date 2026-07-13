//! "Sign in with Microsoft" / "Sign in with Google" account linking.
//!
//! Command: `account_status` | risk: `safe-read` | reads: linked-account
//! metadata only | mutates: no | network: no | approval: none.
//!
//! Command: `account_sign_in` | risk: `external-mutate` | reads: nothing
//! sensitive | mutates: writes the linked-account record | network: yes
//! (opens the system browser, then calls the provider's token + userinfo
//! endpoints) | approval: implicit in the user clicking "Sign in" and
//! completing the provider's own consent screen | audit: not yet logged to
//! the Organizer audit trail (no file/organizer state changes) | undo: n/a
//! (`account_sign_out` reverses it).
//!
//! Command: `account_sign_out` | risk: `local-mutate` | mutates: deletes the
//! linked-account record | network: no (local unlink only; does not revoke
//! the token at the provider — see `accounts.rs`).
//!
//! This is sign-in as an identity, not a data-access grant: linking an
//! account does not itself move workflow/organizer data anywhere. Actual
//! stack integrations (Fabric/Power BI, Google Cloud, AI-assistant
//! connectors) are future, separately-scoped work — see
//! `docs/integrations-roadmap.md`.

use crate::accounts::{AccountManager, AccountRecord};
use crate::core::oauth::Provider;
use crate::engine::GhostEngine;
use tauri::State;

#[derive(serde::Serialize, Default)]
pub struct AccountStatus {
    pub signed_in: bool,
    pub provider: Option<String>,
    pub email: Option<String>,
    pub name: Option<String>,
    /// Whether "Sign in with Google" can run (bundled or configured client ID).
    pub google_sign_in_available: bool,
    /// Whether "Sign in with Microsoft" can run (requires operator Entra app).
    pub microsoft_sign_in_available: bool,
}

impl AccountStatus {
    fn with_availability(integrations: &crate::config::IntegrationSettings) -> Self {
        use crate::core::oauth::Provider;
        Self {
            google_sign_in_available: Provider::parse("google")
                .ok()
                .and_then(|p| p.client_id(integrations).ok())
                .is_some(),
            microsoft_sign_in_available: Provider::parse("microsoft")
                .ok()
                .and_then(|p| p.client_id(integrations).ok())
                .is_some(),
            ..Self::default()
        }
    }
}

impl From<AccountRecord> for AccountStatus {
    fn from(record: AccountRecord) -> Self {
        AccountStatus {
            signed_in: true,
            provider: Some(record.provider),
            email: Some(record.email),
            name: Some(record.name),
            google_sign_in_available: true,
            microsoft_sign_in_available: true,
        }
    }
}

/// Whether an account is currently linked, and who.
#[tauri::command]
pub fn account_status(engine: State<GhostEngine>) -> AccountStatus {
    let integrations = &engine.get_config().integrations;
    engine
        .accounts()
        .current(&engine.auth())
        .map(|record| {
            let mut status = AccountStatus::from(record);
            status.google_sign_in_available = Provider::parse("google")
                .ok()
                .and_then(|p| p.client_id(integrations).ok())
                .is_some();
            status.microsoft_sign_in_available = Provider::parse("microsoft")
                .ok()
                .and_then(|p| p.client_id(integrations).ok())
                .is_some();
            status
        })
        .unwrap_or_else(|| AccountStatus::with_availability(integrations))
}

/// Run the interactive OAuth + PKCE sign-in flow for `provider`
/// ("microsoft" or "google"), then persist the linked account.
///
/// Blocks on network + browser interaction, so it runs on a blocking thread
/// rather than the async command's own task — `run_sign_in_flow` opens a
/// loopback listener and a `reqwest::blocking` client, neither of which
/// belongs on an async executor thread.
#[tauri::command]
pub async fn account_sign_in(
    provider: String,
    engine: State<'_, GhostEngine>,
) -> Result<AccountStatus, String> {
    let provider = Provider::parse(&provider)?;
    let client_id = provider.client_id(&engine.get_config().integrations)?;

    let result = tauri::async_runtime::spawn_blocking(move || {
        AccountManager::run_sign_in(provider, &client_id)
    })
    .await
    .map_err(|e| format!("sign-in task failed: {e}"))?
    .map_err(|e| e.to_string())?;

    engine
        .accounts()
        .store_sign_in_result(&engine.auth(), result)
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("locked") {
                "Unlock Ghost with your password before linking an account.".to_string()
            } else {
                msg
            }
        })?;

    let record = engine
        .accounts()
        .current(&engine.auth())
        .ok_or_else(|| "Sign-in succeeded but account could not be loaded".to_string())?;

    Ok(AccountStatus::from(record))
}

/// Unlink the local account record. Does not revoke provider-side consent.
#[tauri::command]
pub fn account_sign_out(engine: State<GhostEngine>) -> Result<(), String> {
    engine.accounts().clear().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tauri::Manager;

    fn managed_test_app() -> tauri::App<tauri::test::MockRuntime> {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let auth_path = std::env::temp_dir().join(format!(
            "ghost_account_cmd_test_{}_{}.json",
            std::process::id(),
            n
        ));
        let app = tauri::test::mock_app();
        app.manage(GhostEngine::with_auth_path(auth_path));
        app
    }

    #[test]
    fn no_account_reports_signed_out() {
        let app = managed_test_app();
        let status = account_status(app.state());
        assert!(!status.signed_in);
        assert!(status.email.is_none());
        assert!(status.google_sign_in_available);
        assert!(!status.microsoft_sign_in_available);
    }

    #[test]
    fn sign_out_with_no_account_does_not_error() {
        let app = managed_test_app();
        assert!(account_sign_out(app.state()).is_ok());
    }

    #[test]
    fn account_status_reflects_a_stored_link() {
        let app = managed_test_app();
        let engine = app.state::<GhostEngine>();
        let record = AccountRecord {
            provider: "google".to_string(),
            email: "user@example.com".to_string(),
            name: "User".to_string(),
            refresh_token: None,
            linked_at: chrono::Utc::now(),
        };
        engine.accounts().store(&engine.auth(), &record).unwrap();

        let status = account_status(app.state());
        assert!(status.signed_in);
        assert_eq!(status.provider.as_deref(), Some("google"));
        assert_eq!(status.email.as_deref(), Some("user@example.com"));

        account_sign_out(app.state()).unwrap();
        assert!(!account_status(app.state()).signed_in);
    }

    #[test]
    fn account_status_reflects_availability_when_signed_in() {
        let app = managed_test_app();
        let engine = app.state::<GhostEngine>();
        let record = AccountRecord {
            provider: "google".to_string(),
            email: "user@example.com".to_string(),
            name: "User".to_string(),
            refresh_token: None,
            linked_at: chrono::Utc::now(),
        };
        engine.accounts().store(&engine.auth(), &record).unwrap();

        let status = account_status(app.state());
        assert!(status.signed_in);
        assert!(status.google_sign_in_available);
        assert!(!status.microsoft_sign_in_available);
    }

    #[test]
    fn account_status_microsoft_available_when_configured() {
        use crate::test_support::EnvVarGuard;

        let _guard = EnvVarGuard::set("GHOST_MS_CLIENT_ID", "test-ms-client-id");
        let app = managed_test_app();
        let status = account_status(app.state());
        assert!(!status.signed_in);
        assert!(status.google_sign_in_available);
        assert!(status.microsoft_sign_in_available);
    }
}
