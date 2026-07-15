//! Optional pairing gate for local MCP stdio clients.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PairingConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub code: String,
}

fn config_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ghost")
        .join("mcp-pairing.json")
}

pub fn load_pairing_config() -> PairingConfig {
    let path = config_path();
    if let Ok(bytes) = fs::read(&path)
        && let Ok(cfg) = serde_json::from_slice::<PairingConfig>(&bytes)
    {
        return cfg;
    }
    PairingConfig::default()
}

fn save_pairing_config(cfg: &PairingConfig) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_vec_pretty(cfg) {
        let _ = fs::write(path, json);
    }
}

pub fn pairing_is_required() -> bool {
    let cfg = load_pairing_config();
    cfg.enabled && !cfg.code.is_empty()
}

pub fn verify_pairing_code(code: &str) -> bool {
    let cfg = load_pairing_config();
    if !cfg.enabled {
        return true;
    }
    if cfg.code.is_empty() {
        return false;
    }
    constant_time_eq(code.as_bytes(), cfg.code.as_bytes())
}

pub fn enable_pairing() -> String {
    let code = generate_code();
    let cfg = PairingConfig {
        enabled: true,
        code: code.clone(),
    };
    save_pairing_config(&cfg);
    code
}

pub fn disable_pairing() {
    save_pairing_config(&PairingConfig::default());
}

pub fn pairing_status() -> PairingConfig {
    let mut cfg = load_pairing_config();
    if cfg.enabled && !cfg.code.is_empty() {
        cfg.code = mask_code(&cfg.code);
    }
    cfg
}

fn generate_code() -> String {
    use aes_gcm::aead::OsRng;
    use aes_gcm::aead::rand_core::RngCore;
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut bytes = [0u8; 8];
    OsRng.fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
        .collect()
}

fn mask_code(code: &str) -> String {
    if code.len() <= 4 {
        return "****".to_string();
    }
    format!("{}****{}", &code[..2], &code[code.len() - 2..])
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_pairing_allows_empty_code() {
        disable_pairing();
        assert!(!pairing_is_required());
        assert!(verify_pairing_code(""));
    }

    #[test]
    fn mask_code_hides_the_middle_of_the_secret() {
        let masked = mask_code("ABCD2345");
        assert!(masked.starts_with("AB"));
        assert!(masked.ends_with("45"));
        assert!(!masked.contains("CD23"));
        // Short codes are fully masked rather than leaking their length shape.
        assert_eq!(mask_code("AB"), "****");
    }

    #[test]
    fn constant_time_eq_rejects_length_and_content_mismatches() {
        assert!(constant_time_eq(b"code-1234", b"code-1234"));
        assert!(!constant_time_eq(b"code-1234", b"code-1235"));
        assert!(!constant_time_eq(b"code-1234", b"code-123"));
        assert!(!constant_time_eq(b"", b"x"));
    }

    #[test]
    fn generated_codes_use_the_unambiguous_alphabet() {
        // No 0/O/1/I in the alphabet — codes get read aloud/typed by users.
        let code = generate_code();
        assert_eq!(code.len(), 8);
        assert!(code.chars().all(|c| !matches!(c, '0' | 'O' | '1' | 'I')));
    }
}
