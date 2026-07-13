use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalTokenClaims {
    pub plan_id: String,
    pub plan_hash: String,
    pub account_id: Option<String>,
    pub approved_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub nonce: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedApprovalToken {
    pub claims: ApprovalTokenClaims,
    pub signature: String,
}

pub fn claims_are_expired(claims: &ApprovalTokenClaims, now: DateTime<Utc>) -> bool {
    now >= claims.expires_at
}

fn signing_key_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ghost")
        .join("mcp-signing.key")
}

/// Process-wide cache of the signing key. Signing and verification must use
/// the *same* key bytes; without this cache two threads that both find the key
/// file missing on first use would each generate and write a different key,
/// and a token signed under the first key would fail verification under the
/// second. Loading once and reusing it also avoids a filesystem read per token.
fn signing_key() -> Vec<u8> {
    static KEY: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();
    KEY.get_or_init(|| Mutex::new(load_or_create_signing_key()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn load_or_create_signing_key() -> Vec<u8> {
    let path = signing_key_path();
    if let Ok(bytes) = fs::read(&path) {
        if !bytes.is_empty() {
            return bytes;
        }
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let key: [u8; 32] = rand_bytes();
    let _ = fs::write(&path, key);
    key.to_vec()
}

fn rand_bytes() -> [u8; 32] {
    use aes_gcm::aead::rand_core::RngCore;
    use aes_gcm::aead::OsRng;
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

fn sign_claims(claims: &ApprovalTokenClaims, key: &[u8]) -> String {
    let payload = serde_json::to_string(claims).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(key);
    hasher.update(payload.as_bytes());
    let hash = hasher.finalize();
    format!(
        "sha256:{}",
        hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

pub fn issue_approval_token(plan_id: &str, plan_hash: &str) -> SignedApprovalToken {
    let now = Utc::now();
    let claims = ApprovalTokenClaims {
        plan_id: plan_id.to_string(),
        plan_hash: plan_hash.to_string(),
        account_id: None,
        approved_at: now,
        expires_at: now + Duration::minutes(5),
        nonce: uuid::Uuid::new_v4().to_string(),
    };
    let signature = sign_claims(&claims, &signing_key());
    SignedApprovalToken { claims, signature }
}

pub fn verify_execution_token(
    token_json: &str,
    zone_id: &str,
) -> Result<ApprovalTokenClaims, String> {
    verify_execution_token_with_hash(token_json, zone_id, None)
}

/// Verify signature, expiry, zone binding, optional plan hash, and single-use nonce.
pub fn verify_execution_token_with_hash(
    token_json: &str,
    zone_id: &str,
    expected_plan_hash: Option<&str>,
) -> Result<ApprovalTokenClaims, String> {
    let signed: SignedApprovalToken = serde_json::from_str(token_json)
        .map_err(|_| "Invalid approval token format".to_string())?;
    let expected = sign_claims(&signed.claims, &signing_key());
    if expected != signed.signature {
        return Err("Approval token signature is invalid".to_string());
    }
    if claims_are_expired(&signed.claims, Utc::now()) {
        return Err("Approval token has expired".to_string());
    }
    if !signed.claims.plan_id.contains(zone_id) {
        return Err("Approval token does not match the requested Zone".to_string());
    }
    if let Some(expected_hash) = expected_plan_hash {
        if signed.claims.plan_hash != expected_hash {
            return Err("Plan has changed since approval — request a new token".to_string());
        }
    }
    super::token_store::consume_nonce(&signed.claims.nonce)?;
    Ok(signed.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_claims_detected() {
        let claims = ApprovalTokenClaims {
            plan_id: "plan_1".to_string(),
            plan_hash: "sha256:abc".to_string(),
            account_id: None,
            approved_at: Utc::now() - Duration::minutes(10),
            expires_at: Utc::now() - Duration::minutes(5),
            nonce: "nonce".to_string(),
        };
        assert!(claims_are_expired(&claims, Utc::now()));
    }

    #[test]
    fn sign_and_verify_round_trip() {
        let signed = issue_approval_token("plan_zone-1", "sha256:deadbeef");
        let json = serde_json::to_string(&signed).unwrap();
        assert!(verify_execution_token(&json, "zone-1").is_ok());
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let mut signed = issue_approval_token("plan_zone-tamper", "sha256:abc");
        signed.signature = "sha256:0000000000000000".to_string();
        let json = serde_json::to_string(&signed).unwrap();
        let err = verify_execution_token(&json, "zone-tamper").unwrap_err();
        assert!(err.contains("signature is invalid"), "got: {err}");
    }

    #[test]
    fn token_for_a_different_zone_is_rejected() {
        let signed = issue_approval_token("plan_zone-a", "sha256:abc");
        let json = serde_json::to_string(&signed).unwrap();
        let err = verify_execution_token(&json, "zone-b").unwrap_err();
        assert!(
            err.contains("does not match the requested Zone"),
            "got: {err}"
        );
    }

    #[test]
    fn malformed_token_json_is_rejected() {
        let err = verify_execution_token("not-a-token", "zone-x").unwrap_err();
        assert!(err.contains("Invalid approval token format"), "got: {err}");
    }

    #[test]
    fn a_rejected_verification_does_not_burn_the_single_use_nonce() {
        // A token presented against the wrong Zone must fail *before* the nonce
        // is consumed, so an honest client that then presents it against the
        // correct Zone still succeeds. If the reject path burned the nonce, a
        // stray/duplicated request could permanently lock out a valid approval.
        let signed = issue_approval_token("plan_zone-retry", "sha256:abc");
        let json = serde_json::to_string(&signed).unwrap();

        assert!(verify_execution_token(&json, "wrong-zone").is_err());
        // Same token, correct Zone — the nonce was never consumed, so this works.
        assert!(verify_execution_token(&json, "zone-retry").is_ok());
        // And now it is single-use: a replay against the correct Zone fails.
        let err = verify_execution_token(&json, "zone-retry").unwrap_err();
        assert!(err.contains("already been used"), "got: {err}");
    }
}
