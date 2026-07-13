//! Fabric inbound webhook secret persistence (local-only).

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct SecretStore {
    secret: Option<String>,
}

static STORE: Mutex<Option<SecretStore>> = Mutex::new(None);

fn store_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ghost")
        .join("fabric-webhook-secret.json")
}

fn with_store<F, T>(f: F) -> T
where
    F: FnOnce(&mut SecretStore) -> T,
{
    let mut guard = STORE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let path = store_path();
        *guard = Some(if let Ok(bytes) = fs::read(&path) {
            serde_json::from_slice(&bytes).unwrap_or_default()
        } else {
            SecretStore::default()
        });
    }
    let store = guard.get_or_insert_with(SecretStore::default);
    let result = f(store);
    if let Some(parent) = store_path().parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_vec(store) {
        let _ = fs::write(store_path(), json);
    }
    result
}

/// Generate and persist a new webhook secret, returning the full value once.
pub fn generate_secret() -> String {
    let secret = uuid::Uuid::new_v4().to_string();
    with_store(|store| {
        store.secret = Some(secret.clone());
    });
    secret
}

pub fn current_secret() -> Option<String> {
    with_store(|store| store.secret.clone())
}

pub fn clear_secret() {
    with_store(|store| store.secret = None);
}

/// First four characters for UI hint — never log the full secret.
pub fn secret_hint() -> Option<String> {
    current_secret().map(|s| {
        if s.len() <= 4 {
            "****".to_string()
        } else {
            format!("{}…", &s[..4])
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_read_round_trip() {
        clear_secret();
        let s = generate_secret();
        assert_eq!(current_secret().as_deref(), Some(s.as_str()));
        clear_secret();
    }
}
