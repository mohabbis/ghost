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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingApprovalRequest {
    pub request_id: String,
    pub zone_id: String,
    pub plan_id: String,
    pub plan_hash: String,
    pub status: ApprovalRequestStatus,
    pub requested_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<DateTime<Utc>>,
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
    let now = Utc::now();
    let request = PendingApprovalRequest {
        request_id: uuid::Uuid::new_v4().to_string(),
        zone_id: zone_id.to_string(),
        plan_id: format!("plan_{zone_id}"),
        plan_hash: plan_hash.to_string(),
        status: ApprovalRequestStatus::Pending,
        requested_at: now,
        expires_at: now + Duration::minutes(30),
        approved_at: None,
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

/// Mark the newest pending request for a Zone as approved (token issued).
pub fn mark_approved_for_zone(zone_id: &str) {
    with_store(|store| {
        let now = Utc::now();
        let mut pending: Vec<_> = store
            .requests
            .values_mut()
            .filter(|r| r.zone_id == zone_id && r.status == ApprovalRequestStatus::Pending)
            .collect();
        pending.sort_by_key(|r| std::cmp::Reverse(r.requested_at));
        if let Some(req) = pending.into_iter().next() {
            req.status = ApprovalRequestStatus::Approved;
            req.approved_at = Some(now);
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
    fn mark_approved_for_zone_updates_status() {
        let req = create_request("zone-b", "sha256:def");
        mark_approved_for_zone("zone-b");
        let loaded = get_request(&req.request_id).expect("request stored");
        assert_eq!(loaded.status, ApprovalRequestStatus::Approved);
        assert!(loaded.approved_at.is_some());
    }
}
