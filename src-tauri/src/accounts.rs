//! Persisted OAuth account link (`commands/account.rs`), backed by `identity`.
//!
//! Signing in establishes identity only. Integration grants (Fabric, Power BI,
//! etc.) are stored separately in `IdentityStore`. See `docs/integrations-roadmap.md`.

use crate::auth::AuthManager;
use crate::identity::{
    AccountIdentity, IdentityStore, OAuthProvider, TokenMaterial, run_sign_in_flow,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// UI-facing account summary (no tokens).
#[derive(Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    /// "microsoft" | "google"
    pub provider: String,
    pub email: String,
    pub name: String,
    /// Deprecated: tokens live in `IdentityStore` token records. Kept for test fixtures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub linked_at: chrono::DateTime<chrono::Utc>,
}

impl From<AccountIdentity> for AccountRecord {
    fn from(identity: AccountIdentity) -> Self {
        AccountRecord {
            provider: identity.provider.as_str().to_string(),
            email: identity.email,
            name: identity.display_name,
            refresh_token: None,
            linked_at: identity.linked_at,
        }
    }
}

pub struct AccountManager {
    store: IdentityStore,
    /// Legacy path retained for migration tests only.
    #[allow(dead_code)]
    legacy_path: PathBuf,
}

impl AccountManager {
    pub fn new() -> Self {
        let store = IdentityStore::new();
        let legacy_path = store
            .path()
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("account.json");
        Self { store, legacy_path }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self {
            legacy_path: path.clone(),
            store: IdentityStore::with_path(path),
        }
    }

    pub fn identity_store(&self) -> &IdentityStore {
        &self.store
    }

    pub fn current(&self, auth: &AuthManager) -> Option<AccountRecord> {
        self.store.identity(auth).map(AccountRecord::from)
    }

    pub fn store(&self, auth: &AuthManager, record: &AccountRecord) -> anyhow::Result<()> {
        let provider = crate::identity::IdentityProvider::parse(&record.provider)
            .map_err(|e| anyhow::anyhow!(e))?;
        let identity = AccountIdentity {
            account_id: uuid::Uuid::new_v4().to_string(),
            provider,
            subject: record.email.clone(),
            tenant_id: None,
            email: record.email.clone(),
            display_name: record.name.clone(),
            linked_at: record.linked_at,
        };
        let scopes: Vec<&str> = match provider {
            crate::identity::IdentityProvider::Microsoft => {
                vec!["openid", "email", "profile", "offline_access"]
            }
            crate::identity::IdentityProvider::Google => {
                vec!["openid", "email", "profile"]
            }
        };
        self.store.store_sign_in(
            auth,
            identity,
            &scopes,
            TokenMaterial {
                access_token: String::new(),
                refresh_token: record.refresh_token.clone(),
                expires_at: None,
            },
        )
    }

    pub fn store_sign_in_result(
        &self,
        auth: &AuthManager,
        result: crate::identity::oauth::flow::SignInResult,
    ) -> anyhow::Result<()> {
        let scope_refs: Vec<&str> = result.scopes;
        self.store
            .store_sign_in(auth, result.identity, &scope_refs, result.tokens)
    }

    pub fn run_sign_in(
        provider: OAuthProvider,
        client_id: &str,
    ) -> anyhow::Result<crate::identity::oauth::flow::SignInResult> {
        run_sign_in_flow(provider, client_id)
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        self.store.clear()
    }

    #[cfg(test)]
    pub fn legacy_path(&self) -> &PathBuf {
        &self.legacy_path
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
            AccountManager::with_path(dir.join("identity.json")),
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

        let raw = std::fs::read_to_string(accounts.identity_store().path()).unwrap();
        assert!(!raw.contains("refresh-token-value"));

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn store_encrypts_at_rest_once_vault_password_is_configured() {
        let (accounts, auth, dir) = temp_managers();
        auth.setup("correct horse battery").unwrap();
        accounts.store(&auth, &sample_record()).unwrap();

        let raw = std::fs::read_to_string(accounts.identity_store().path()).unwrap();
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

        accounts.clear().unwrap();

        std::fs::remove_dir_all(dir).ok();
    }
}
