//! Local authentication and at-rest encryption for workflow data.
//!
//! Ghost is local-only: there are no accounts and no server. "Login" means a
//! password the user creates on their machine. From it we derive a key with
//! Argon2id and use AES-256-GCM to encrypt workflow files, so the promise on
//! the website — "your data never leaves your device" — stays true while the
//! data is also unreadable without the password.
//!
//! Key hierarchy (standard envelope encryption):
//! - A random 32-byte data-encryption key (DEK) encrypts workflow files.
//! - The DEK itself is wrapped (encrypted) with a key derived from the user's
//!   password via Argon2id. Changing the password later only needs re-wrapping
//!   the DEK, not re-encrypting every workflow.
//! - Unlocking = unwrapping the DEK into memory. AES-GCM is authenticated, so
//!   a wrong password fails the tag check — no separate verifier is stored.
//!
//! If no password is configured, Ghost behaves exactly as before (plaintext
//! workflow files); the walkthrough lets users skip protection.

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::Argon2;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const MIN_PASSWORD_LEN: usize = 8;

/// On-disk record of the wrapped data-encryption key. Contains no secrets
/// recoverable without the password.
#[derive(Serialize, Deserialize)]
struct AuthFile {
    version: u32,
    kdf: String,
    salt: String,        // base64
    dek_nonce: String,   // base64
    wrapped_dek: String, // base64 (AES-256-GCM ciphertext + tag)
}

/// JSON envelope written in place of plaintext workflow JSON once a password
/// is configured. The `ghost_encrypted` field doubles as a format marker so
/// pre-existing plaintext files are still recognized and loaded.
#[derive(Serialize, Deserialize)]
struct EncryptedEnvelope {
    ghost_encrypted: u32,
    nonce: String, // base64
    data: String,  // base64 ciphertext
}

pub struct AuthManager {
    auth_path: PathBuf,
    /// Unwrapped data-encryption key, present only while unlocked.
    dek: Mutex<Option<[u8; KEY_LEN]>>,
}

impl AuthManager {
    /// Manager rooted at the standard data directory (data_dir/ghost/auth.json).
    pub fn new() -> Self {
        let auth_path = dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("ghost")
            .join("auth.json");
        Self::with_path(auth_path)
    }

    /// Manager with an explicit auth-file path (used by tests).
    pub fn with_path(auth_path: PathBuf) -> Self {
        AuthManager {
            auth_path,
            dek: Mutex::new(None),
        }
    }

    /// Whether a password has been set up on this machine.
    pub fn is_configured(&self) -> bool {
        self.auth_path.exists()
    }

    /// Whether workflow data is currently accessible. An unconfigured
    /// installation is always "unlocked" (no protection requested).
    pub fn is_unlocked(&self) -> bool {
        !self.is_configured() || self.dek.lock().unwrap().is_some()
    }

    /// Create the password, generate and wrap the DEK, and leave the app
    /// unlocked. Fails if a password already exists.
    pub fn setup(&self, password: &str) -> anyhow::Result<()> {
        if self.is_configured() {
            anyhow::bail!("A password is already configured");
        }
        if password.len() < MIN_PASSWORD_LEN {
            anyhow::bail!("Password must be at least {} characters", MIN_PASSWORD_LEN);
        }

        let mut salt = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        let mut dek = [0u8; KEY_LEN];
        OsRng.fill_bytes(&mut dek);
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);

        let kek = derive_key(password, &salt)?;
        let cipher = Aes256Gcm::new_from_slice(&kek)
            .map_err(|e| anyhow::anyhow!("cipher init failed: {e}"))?;
        let wrapped = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), dek.as_slice())
            .map_err(|_| anyhow::anyhow!("failed to wrap data key"))?;

        let record = AuthFile {
            version: 1,
            kdf: "argon2id".to_string(),
            salt: B64.encode(salt),
            dek_nonce: B64.encode(nonce_bytes),
            wrapped_dek: B64.encode(wrapped),
        };

        if let Some(parent) = self.auth_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.auth_path, serde_json::to_string_pretty(&record)?)?;

        *self.dek.lock().unwrap() = Some(dek);
        Ok(())
    }

    /// Try to unlock with the given password. Returns Ok(false) on a wrong
    /// password (the GCM tag check fails), Err only on real I/O/format errors.
    pub fn unlock(&self, password: &str) -> anyhow::Result<bool> {
        if !self.is_configured() {
            // Nothing to unlock — treat as success so callers don't special-case.
            return Ok(true);
        }

        let raw = std::fs::read_to_string(&self.auth_path)?;
        let record: AuthFile = serde_json::from_str(&raw)?;
        let salt = B64.decode(&record.salt)?;
        let nonce_bytes = B64.decode(&record.dek_nonce)?;
        let wrapped = B64.decode(&record.wrapped_dek)?;

        let kek = derive_key(password, &salt)?;
        let cipher = Aes256Gcm::new_from_slice(&kek)
            .map_err(|e| anyhow::anyhow!("cipher init failed: {e}"))?;

        match cipher.decrypt(Nonce::from_slice(&nonce_bytes), wrapped.as_slice()) {
            Ok(dek_vec) if dek_vec.len() == KEY_LEN => {
                let mut dek = [0u8; KEY_LEN];
                dek.copy_from_slice(&dek_vec);
                *self.dek.lock().unwrap() = Some(dek);
                Ok(true)
            }
            Ok(_) => anyhow::bail!("corrupt auth file: unexpected key length"),
            Err(_) => Ok(false), // wrong password
        }
    }

    /// Drop the in-memory key; workflow data becomes inaccessible until unlock.
    pub fn lock(&self) {
        *self.dek.lock().unwrap() = None;
    }

    /// Prepare a JSON string for writing to disk: encrypted when a password is
    /// configured, plaintext passthrough otherwise.
    pub fn protect(&self, json: &str) -> anyhow::Result<String> {
        if !self.is_configured() {
            return Ok(json.to_string());
        }
        let dek = self.require_dek()?;

        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let cipher = Aes256Gcm::new_from_slice(&dek)
            .map_err(|e| anyhow::anyhow!("cipher init failed: {e}"))?;
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), json.as_bytes())
            .map_err(|_| anyhow::anyhow!("encryption failed"))?;

        let envelope = EncryptedEnvelope {
            ghost_encrypted: 1,
            nonce: B64.encode(nonce_bytes),
            data: B64.encode(ciphertext),
        };
        Ok(serde_json::to_string(&envelope)?)
    }

    /// Inverse of `protect`: decrypt if the content is an encrypted envelope,
    /// otherwise return it unchanged (pre-password plaintext workflows).
    pub fn reveal(&self, content: &str) -> anyhow::Result<String> {
        let envelope: EncryptedEnvelope = match serde_json::from_str(content) {
            Ok(env) => env,
            Err(_) => return Ok(content.to_string()), // plaintext file
        };

        let dek = self.require_dek()?;
        let nonce_bytes = B64.decode(&envelope.nonce)?;
        let ciphertext = B64.decode(&envelope.data)?;
        let cipher = Aes256Gcm::new_from_slice(&dek)
            .map_err(|e| anyhow::anyhow!("cipher init failed: {e}"))?;
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&nonce_bytes), ciphertext.as_slice())
            .map_err(|_| anyhow::anyhow!("decryption failed — file may be corrupt"))?;
        Ok(String::from_utf8(plaintext)?)
    }

    fn require_dek(&self) -> anyhow::Result<[u8; KEY_LEN]> {
        self.dek.lock().unwrap().ok_or_else(|| {
            anyhow::anyhow!("Ghost is locked — unlock with your password to access workflows")
        })
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

fn derive_key(password: &str, salt: &[u8]) -> anyhow::Result<[u8; KEY_LEN]> {
    let mut key = [0u8; KEY_LEN];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| anyhow::anyhow!("key derivation failed: {e}"))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_manager() -> (AuthManager, PathBuf) {
        let dir = std::env::temp_dir().join(format!("ghost-auth-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        (AuthManager::with_path(dir.join("auth.json")), dir)
    }

    #[test]
    fn unconfigured_is_plaintext_passthrough() {
        let (auth, dir) = temp_manager();
        assert!(!auth.is_configured());
        assert!(auth.is_unlocked());

        let json = r#"[{"Delay":{"ms":100}}]"#;
        assert_eq!(auth.protect(json).unwrap(), json);
        assert_eq!(auth.reveal(json).unwrap(), json);

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn setup_unlock_roundtrip_and_wrong_password() {
        let (auth, dir) = temp_manager();
        auth.setup("correct horse battery").unwrap();
        assert!(auth.is_configured());
        assert!(auth.is_unlocked());

        // Re-setup must be rejected.
        assert!(auth.setup("another password").is_err());

        auth.lock();
        assert!(!auth.is_unlocked());
        assert!(!auth.unlock("wrong password!").unwrap());
        assert!(!auth.is_unlocked());
        assert!(auth.unlock("correct horse battery").unwrap());
        assert!(auth.is_unlocked());

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn short_password_rejected() {
        let (auth, dir) = temp_manager();
        assert!(auth.setup("short").is_err());
        assert!(!auth.is_configured());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn protect_reveal_roundtrip_and_locked_access() {
        let (auth, dir) = temp_manager();
        auth.setup("a strong password").unwrap();

        let json = r#"{"name":"demo","events":[]}"#;
        let stored = auth.protect(json).unwrap();
        assert_ne!(stored, json);
        assert!(stored.contains("ghost_encrypted"));
        assert!(!stored.contains("demo")); // no plaintext leakage
        assert_eq!(auth.reveal(&stored).unwrap(), json);

        // Plaintext files written before the password existed still load.
        assert_eq!(auth.reveal(json).unwrap(), json);

        // Locked: writing and reading protected data must fail loudly.
        auth.lock();
        assert!(auth.protect(json).is_err());
        assert!(auth.reveal(&stored).is_err());

        std::fs::remove_dir_all(dir).ok();
    }
}
