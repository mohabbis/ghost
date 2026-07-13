use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::OsRng;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use sha2::{Digest, Sha256};

pub fn pkce_pair() -> (String, String) {
    let mut verifier_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut verifier_bytes);
    let verifier = B64URL.encode(verifier_bytes);
    let challenge = B64URL.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

pub fn random_state() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    B64URL.encode(bytes)
}

/// Constant-time equality check for the CSRF `state` round-trip. A plain
/// `==` short-circuits on the first mismatched byte, which is the standard
/// textbook timing side channel for security-token comparison — cheap
/// enough to just not have, even though the practical exploitability here is
/// low (a single-shot, short-window local flow, not a repeatable oracle).
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_sha256_of_verifier() {
        let (verifier, challenge) = pkce_pair();
        let expected = B64URL.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, expected);
        assert_ne!(verifier, challenge);
    }

    #[test]
    fn constant_time_eq_matches_plain_equality_semantics() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("abc123", "abc12"));
        assert!(!constant_time_eq("abc123", ""));
        assert!(constant_time_eq("", ""));
    }
}
