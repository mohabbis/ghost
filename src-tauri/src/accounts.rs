//! Persisted OAuth account link (`commands/account.rs`), independent of the
//! local vault password (`auth.rs`).
//!
//! Signing in with Microsoft or Google answers "who is the user" — it does
//! not gate access to Ghost, does not replace the vault password, and does
//! not by itself move any workflow/organizer data anywhere. It exists so a
//! later, separately-scoped integration (Fabric/Power BI, Google Cloud, an
//! AI-assistant connector) can request access under a known identity instead
//! of Ghost inventing its own account system. See
//! `docs/integrations-roadmap.md`.
//!
//! The stored record (including the refresh token, if the provider issued
//! one) is run through the same `AuthManager::protect`/`reveal` envelope as
//! workflow files: encrypted at rest once a vault password is configured,
//! plaintext-on-disk otherwise — same trust model, not a stronger or weaker
//! one for account data specifically.

use crate::auth::AuthManager;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    /// "microsoft" | "google"
    pub provider: String,
    pub email: String,
    pub name: String,
    /// Present when the provider issued one (Microsoft with `offline_access`;
    /// Google only on first consent). Never sent anywhere except back to the
    /// issuing provider's token endpoint by a future integration.
    pub refresh_token: Option<String>,
    pub linked_at: chrono::DateTime<chrono::Utc>,
}

pub struct AccountManager {
    path: PathBuf,
}

impl AccountManager {
    pub fn new() -> Self {
        let path = dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("ghost")
            .join("account.json");
        Self::with_path(path)
    }

    pub fn with_path(path: PathBuf) -> Self {
        AccountManager { path }
    }

    /// The currently linked account, if any. Returns `None` (not an error)
    /// both when no account was ever linked and when the vault is locked —
    /// callers treat "can't tell right now" the same as "not signed in" for
    /// display purposes.
    pub fn current(&self, auth: &AuthManager) -> Option<AccountRecord> {
        let raw = std::fs::read_to_string(&self.path).ok()?;
        let revealed = auth.reveal(&raw).ok()?;
        serde_json::from_str(&revealed).ok()
    }

    /// Persist a newly linked account, overwriting any previous link (a
    /// second sign-in replaces the first rather than stacking accounts).
    pub fn store(&self, auth: &AuthManager, record: &AccountRecord) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(record)?;
        let protected = auth.protect(&json)?;
        crate::core::security::atomic_write(&self.path, protected.as_bytes())?;
        Ok(())
    }

    /// Unlink the local record. This does not revoke the token at the
    /// provider — the user should also remove Ghost from their Microsoft/
    /// Google account's connected-apps list if they want that.
    pub fn clear(&self) -> anyhow::Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

impl Default for AccountManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_managers() -> (AccountManager, AuthManager, PathBuf) {
        let dir = std::env::temp_dir().join(format!("ghost-account-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        (
            AccountManager::with_path(dir.join("account.json")),
            AuthManager::with_path(dir.join("auth.json")),
            dir,
        )
    }

    fn sample_record() -> AccountRecord {
        AccountRecord {
            provider: "microsoft".to_string(),
            email: "user@example.com".to_string(),
            name: "Sample User".to_string(),
            refresh_token: Some("refresh-token-value".to_string()),
            linked_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn no_link_reports_none() {
        let (accounts, auth, dir) = temp_managers();
        assert!(accounts.current(&auth).is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn store_then_current_round_trips_without_vault_password() {
        let (accounts, auth, dir) = temp_managers();
        accounts.store(&auth, &sample_record()).unwrap();

        let loaded = accounts.current(&auth).unwrap();
        assert_eq!(loaded.email, "user@example.com");
        assert_eq!(loaded.provider, "microsoft");

        // Unconfigured vault: on-disk file must not be an encrypted envelope.
        let raw = std::fs::read_to_string(&accounts.path).unwrap();
        assert!(!raw.contains("ghost_encrypted"));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn store_encrypts_at_rest_once_vault_password_is_configured() {
        let (accounts, auth, dir) = temp_managers();
        auth.setup("correct horse battery").unwrap();
        accounts.store(&auth, &sample_record()).unwrap();

        let raw = std::fs::read_to_string(&accounts.path).unwrap();
        assert!(raw.contains("ghost_encrypted"));
        assert!(!raw.contains("example.com"));

        let loaded = accounts.current(&auth).unwrap();
        assert_eq!(loaded.email, "user@example.com");

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn locked_vault_hides_the_account_without_erroring() {
        let (accounts, auth, dir) = temp_managers();
        auth.setup("correct horse battery").unwrap();
        accounts.store(&auth, &sample_record()).unwrap();

        auth.lock();
        assert!(accounts.current(&auth).is_none());

        auth.unlock("correct horse battery").unwrap();
        assert!(accounts.current(&auth).is_some());

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn clear_removes_the_link() {
        let (accounts, auth, dir) = temp_managers();
        accounts.store(&auth, &sample_record()).unwrap();
        assert!(accounts.current(&auth).is_some());

        accounts.clear().unwrap();
        assert!(accounts.current(&auth).is_none());

        // Clearing again (nothing to remove) must not error.
        accounts.clear().unwrap();

        std::fs::remove_dir_all(dir).ok();
    }
}
