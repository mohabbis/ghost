use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalTokenClaims {
    pub plan_id: String,
    pub plan_hash: String,
    pub account_id: Option<String>,
    pub approved_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub nonce: String,
}

/// Approval token verification is planned; claims shape is stable for docs/tests.
pub fn claims_are_expired(claims: &ApprovalTokenClaims, now: DateTime<Utc>) -> bool {
    now >= claims.expires_at
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
            approved_at: Utc::now() - chrono::Duration::minutes(10),
            expires_at: Utc::now() - chrono::Duration::minutes(5),
            nonce: "nonce".to_string(),
        };
        assert!(claims_are_expired(&claims, Utc::now()));
    }
}
