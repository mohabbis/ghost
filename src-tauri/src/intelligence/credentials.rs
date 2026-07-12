//! Encrypted storage for intelligence provider API keys (never in config.json).

use crate::auth::AuthManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialProvider {
    OpenAi,
    Anthropic,
}

impl CredentialProvider {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "openai" => Ok(CredentialProvider::OpenAi),
            "anthropic" => Ok(CredentialProvider::Anthropic),
            other => Err(format!(
                "Unknown intelligence credential provider '{other}' (expected openai or anthropic)"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CredentialProvider::OpenAi => "openai",
            CredentialProvider::Anthropic => "anthropic",
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct SecretsFile {
    keys: HashMap<String, String>,
}

pub struct CredentialStore {
    path: PathBuf,
}

impl Clone for CredentialStore {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
        }
    }
}

impl CredentialStore {
    pub fn new() -> Self {
        let path = dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("ghost")
            .join("intelligence-secrets.json");
        Self { path }
    }

    pub fn with_path(path: PathBuf) -> Self {
        CredentialStore { path }
    }

    pub fn is_configured(&self, auth: &AuthManager, provider: CredentialProvider) -> bool {
        self.load(auth)
            .map(|f| f.keys.contains_key(provider.as_str()))
            .unwrap_or(false)
    }

    pub fn get(&self, auth: &AuthManager, provider: CredentialProvider) -> Option<String> {
        self.load(auth)
            .ok()
            .and_then(|f| f.keys.get(provider.as_str()).cloned())
    }

    pub fn set(
        &self,
        auth: &AuthManager,
        provider: CredentialProvider,
        api_key: &str,
    ) -> anyhow::Result<()> {
        let trimmed = api_key.trim();
        if trimmed.is_empty() {
            anyhow::bail!("API key cannot be empty");
        }
        let mut file = self.load(auth).unwrap_or_default();
        file.keys
            .insert(provider.as_str().to_string(), trimmed.to_string());
        self.save(auth, &file)
    }

    pub fn clear(&self, auth: &AuthManager, provider: CredentialProvider) -> anyhow::Result<()> {
        let mut file = self.load(auth).unwrap_or_default();
        file.keys.remove(provider.as_str());
        if file.keys.is_empty() {
            return self.delete_file();
        }
        self.save(auth, &file)
    }

    fn load(&self, auth: &AuthManager) -> anyhow::Result<SecretsFile> {
        let raw = match std::fs::read_to_string(&self.path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(SecretsFile::default()),
            Err(e) => return Err(e.into()),
        };
        let revealed = auth.reveal(&raw)?;
        Ok(serde_json::from_str(&revealed)?)
    }

    fn save(&self, auth: &AuthManager, file: &SecretsFile) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(file)?;
        let protected = auth.protect(&json)?;
        crate::core::security::atomic_write(&self.path, protected.as_bytes())?;
        Ok(())
    }

    fn delete_file(&self) -> anyhow::Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

impl Default for CredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (CredentialStore, AuthManager, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("ghost-intel-secrets-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        (
            CredentialStore::with_path(dir.join("intelligence-secrets.json")),
            AuthManager::with_path(dir.join("auth.json")),
            dir,
        )
    }

    #[test]
    fn store_and_load_round_trips_without_vault() {
        let (store, auth, dir) = temp_store();
        store
            .set(&auth, CredentialProvider::OpenAi, "sk-test-key")
            .unwrap();
        assert_eq!(
            store.get(&auth, CredentialProvider::OpenAi).as_deref(),
            Some("sk-test-key")
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn store_encrypts_at_rest_with_vault_password() {
        let (store, auth, dir) = temp_store();
        auth.setup("correct horse battery").unwrap();
        store
            .set(&auth, CredentialProvider::OpenAi, "sk-test-key")
            .unwrap();
        let raw = std::fs::read_to_string(&store.path).unwrap();
        assert!(!raw.contains("sk-test-key"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn clear_removes_one_provider() {
        let (store, auth, dir) = temp_store();
        store
            .set(&auth, CredentialProvider::OpenAi, "sk-openai")
            .unwrap();
        store
            .set(&auth, CredentialProvider::Anthropic, "sk-anthropic")
            .unwrap();
        store.clear(&auth, CredentialProvider::OpenAi).unwrap();
        assert!(!store.is_configured(&auth, CredentialProvider::OpenAi));
        assert!(store.is_configured(&auth, CredentialProvider::Anthropic));
        std::fs::remove_dir_all(dir).ok();
    }
}
