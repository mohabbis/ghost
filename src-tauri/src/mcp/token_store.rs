//! Single-use nonce tracking for MCP approval tokens.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

static USED_NONCES: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn store_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ghost")
        .join("mcp-used-nonces.json")
}

fn load_used() -> HashSet<String> {
    let path = store_path();
    if let Ok(bytes) = fs::read(&path)
        && let Ok(set) = serde_json::from_slice::<HashSet<String>>(&bytes)
    {
        return set;
    }
    HashSet::new()
}

fn persist_used(set: &HashSet<String>) {
    let path = store_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_vec(set) {
        let _ = fs::write(path, json);
    }
}

fn used_nonces() -> HashSet<String> {
    let mut guard = USED_NONCES.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(load_used());
    }
    guard.clone().unwrap_or_default()
}

fn with_used<F, T>(f: F) -> T
where
    F: FnOnce(&mut HashSet<String>) -> T,
{
    let mut guard = USED_NONCES.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(load_used());
    }
    let set = guard.get_or_insert_with(load_used);
    let result = f(set);
    persist_used(set);
    result
}

pub fn nonce_is_consumed(nonce: &str) -> bool {
    used_nonces().contains(nonce)
}

/// Mark a nonce as consumed. Returns an error if it was already used.
pub fn consume_nonce(nonce: &str) -> Result<(), String> {
    with_used(|set| {
        if !set.insert(nonce.to_string()) {
            return Err("Approval token has already been used".to_string());
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consume_nonce_rejects_replay() {
        let nonce = format!("test-nonce-{}", uuid::Uuid::new_v4());
        consume_nonce(&nonce).expect("first consume");
        assert!(consume_nonce(&nonce).is_err());
    }
}
