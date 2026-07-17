//! File-backed pending MCP approval requests shared by the headless MCP server
//! and the Ghost desktop app.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRequestStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRequestKind {
    #[default]
    Organizer,
    Routine,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingApprovalRequest {
    pub request_id: String,
    #[serde(default)]
    pub kind: ApprovalRequestKind,
    #[serde(default)]
    pub zone_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routine_name: Option<String>,
    pub plan_id: String,
    pub plan_hash: String,
    pub status: ApprovalRequestStatus,
    pub requested_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_token: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct Store {
    requests: HashMap<String, PendingApprovalRequest>,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn store_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ghost")
        .join("mcp-pending-approvals.json")
}

fn load_store() -> Store {
    let path = store_path();
    if let Ok(bytes) = fs::read(&path)
        && let Ok(store) = serde_json::from_slice::<Store>(&bytes)
    {
        return store;
    }
    Store::default()
}

fn persist_store(store: &Store) {
    let path = store_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_vec(store) {
        let _ = fs::write(path, json);
    }
}

fn with_store<F, T>(f: F) -> T
where
    F: FnOnce(&mut Store) -> T,
{
    let mut guard = STORE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(load_store());
    }
    let store = guard.get_or_insert_with(load_store);
    refresh_expired(store);
    let result = f(store);
    persist_store(store);
    result
}

fn refresh_expired(store: &mut Store) {
    let now = Utc::now();
    for req in store.requests.values_mut() {
        if req.status == ApprovalRequestStatus::Pending && now >= req.expires_at {
            req.status = ApprovalRequestStatus::Expired;
        }
    }
}

/// Create a pending approval request for a Zone plan hash.
pub fn create_request(zone_id: &str, plan_hash: &str) -> PendingApprovalRequest {
    create_request_for(
        ApprovalRequestKind::Organizer,
        zone_id,
        None,
        &format!("plan_{zone_id}"),
        plan_hash,
    )
}

/// Create a pending approval request for an exact saved routine plan.
pub fn create_routine_request(
    routine_name: &str,
    plan_id: &str,
    plan_hash: &str,
) -> PendingApprovalRequest {
    create_request_for(
        ApprovalRequestKind::Routine,
        "",
        Some(routine_name.to_string()),
        plan_id,
        plan_hash,
    )
}

fn create_request_for(
    kind: ApprovalRequestKind,
    zone_id: &str,
    routine_name: Option<String>,
    plan_id: &str,
    plan_hash: &str,
) -> PendingApprovalRequest {
    let now = Utc::now();
    let request = PendingApprovalRequest {
        request_id: uuid::Uuid::new_v4().to_string(),
        kind,
        zone_id: zone_id.to_string(),
        routine_name,
        plan_id: plan_id.to_string(),
        plan_hash: plan_hash.to_string(),
        status: ApprovalRequestStatus::Pending,
        requested_at: now,
        expires_at: now + Duration::minutes(30),
        approved_at: None,
        approval_token: None,
    };
    with_store(|store| {
        store
            .requests
            .insert(request.request_id.clone(), request.clone());
    });
    request
}

pub fn get_request(request_id: &str) -> Option<PendingApprovalRequest> {
    with_store(|store| store.requests.get(request_id).cloned())
}

pub fn list_pending() -> Vec<PendingApprovalRequest> {
    with_store(|store| {
        store
            .requests
            .values()
            .filter(|r| r.status == ApprovalRequestStatus::Pending)
            .cloned()
            .collect()
    })
}

/// Mark an exact pending request approved after the desktop has re-derived the
/// same plan hash. The signed token is returned only to a client that knows the
/// unguessable request id and polls approval status.
pub fn mark_approved(
    request_id: &str,
    expected_plan_hash: &str,
    approval_token: String,
) -> Result<(), String> {
    with_store(|store| {
        let now = Utc::now();
        let req = store
            .requests
            .get_mut(request_id)
            .ok_or_else(|| format!("Unknown approval request '{request_id}'"))?;
        if req.status != ApprovalRequestStatus::Pending {
            return Err("Approval request is no longer pending".to_string());
        }
        if req.plan_hash != expected_plan_hash {
            return Err("Plan changed since the MCP request — request approval again".to_string());
        }
        req.status = ApprovalRequestStatus::Approved;
        req.approved_at = Some(now);
        req.approval_token = Some(approval_token);
        Ok(())
    })
}

/// Mark the newest pending Organizer request for a Zone as approved.
/// Used by the Organizer "MCP token" button when the UI does not hold a
/// request_id (client may have requested approval separately, or not at all).
pub fn mark_approved_for_zone(zone_id: &str, approval_token: String) {
    with_store(|store| {
        let now = Utc::now();
        let mut pending: Vec<_> = store
            .requests
            .values_mut()
            .filter(|r| {
                r.kind == ApprovalRequestKind::Organizer
                    && r.zone_id == zone_id
                    && r.status == ApprovalRequestStatus::Pending
            })
            .collect();
        pending.sort_by_key(|r| std::cmp::Reverse(r.requested_at));
        if let Some(req) = pending.into_iter().next() {
            req.status = ApprovalRequestStatus::Approved;
            req.approved_at = Some(now);
            req.approval_token = Some(approval_token);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get_request_round_trip() {
        let req = create_request("zone-a", "sha256:abc");
        let loaded = get_request(&req.request_id).expect("request stored");
        assert_eq!(loaded.zone_id, "zone-a");
        assert_eq!(loaded.status, ApprovalRequestStatus::Pending);
    }

    #[test]
    fn mark_approved_updates_exact_request() {
        let req = create_request("zone-b", "sha256:def");
        mark_approved(&req.request_id, "sha256:def", "signed-token".into()).unwrap();
        let loaded = get_request(&req.request_id).expect("request stored");
        assert_eq!(loaded.status, ApprovalRequestStatus::Approved);
        assert!(loaded.approved_at.is_some());
        assert_eq!(loaded.approval_token.as_deref(), Some("signed-token"));
    }

    #[test]
    fn mark_approved_refuses_a_changed_plan_hash() {
        let req = create_routine_request("transfer", "routine:transfer", "sha256:old");
        let err = mark_approved(&req.request_id, "sha256:new", "signed-token".into()).unwrap_err();
        assert!(err.contains("Plan changed"), "got: {err}");
        assert_eq!(
            get_request(&req.request_id).unwrap().status,
            ApprovalRequestStatus::Pending
        );
    }
}
