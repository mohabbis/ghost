//! MCP approval-token integration tests (no live JSON-RPC server).

use chrono::{Duration, Utc};
use ghost_lib::mcp::approval::{
    claims_are_expired, issue_approval_token, verify_execution_token_with_hash, ApprovalTokenClaims,
};
use ghost_lib::mcp::plan_hash::hash_organizer_plan;
use ghost_lib::mcp::token_store::consume_nonce;
use ghost_lib::organizer::planner::plan_with_rules;
use ghost_lib::policy::FolderRule;

#[test]
fn plan_hash_mismatch_rejects_token() {
    let signed = issue_approval_token("plan_zone-test", "sha256:deadbeef");
    let json = serde_json::to_string(&signed).unwrap();
    let err =
        verify_execution_token_with_hash(&json, "zone-test", Some("sha256:other")).unwrap_err();
    assert!(err.contains("changed since approval"));
}

#[test]
fn nonce_replay_rejects_second_consume() {
    let signed = issue_approval_token("plan_zone-replay", "sha256:abc");
    consume_nonce(&signed.claims.nonce).expect("first consume");
    let err = consume_nonce(&signed.claims.nonce).unwrap_err();
    assert!(err.contains("already been used"));
}

#[test]
fn expired_claims_detected() {
    let claims = ApprovalTokenClaims {
        plan_id: "plan_zone-exp".into(),
        plan_hash: "sha256:abc".into(),
        account_id: None,
        approved_at: Utc::now() - Duration::minutes(10),
        expires_at: Utc::now() - Duration::minutes(1),
        nonce: "nonce-exp".into(),
    };
    assert!(claims_are_expired(&claims, Utc::now()));
}

#[test]
fn hash_organizer_plan_is_stable_for_same_plan() {
    let rules = vec![FolderRule {
        path: std::path::PathBuf::from("/tmp"),
        can_read: true,
        can_create: true,
        can_rename: true,
        can_move: true,
        can_copy: true,
        can_delete: false,
        trust: ghost_lib::policy::TrustLevel::AskFirst,
    }];
    let plan = plan_with_rules("z1", &rules);
    let h1 = hash_organizer_plan(&plan);
    let h2 = hash_organizer_plan(&plan);
    assert_eq!(h1, h2);
    assert!(h1.starts_with("sha256:"));
}

#[test]
fn sign_and_verify_round_trip_with_plan_hash() {
    let signed = issue_approval_token("plan_zone-round", "sha256:feed");
    let json = serde_json::to_string(&signed).unwrap();
    verify_execution_token_with_hash(&json, "zone-round", Some("sha256:feed"))
        .expect("valid token");
}
