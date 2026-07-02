//! Cloud sync capabilities for Ghost workflows.
//! Provides secure synchronization across devices.
//!
//! **NOTE:** Cloud sync is currently disabled in this build.
//! All cloud-related methods return errors indicating the feature is unavailable.

use crate::core::events::Workflow;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;
use uuid::Uuid;

/// Cloud sync configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CloudConfig {
    pub api_endpoint: String,
    pub auth_token: Option<String>,
    pub auto_sync: bool,
    pub sync_interval_ms: u64,
}

impl Default for CloudConfig {
    fn default() -> Self {
        CloudConfig {
            api_endpoint: "https://api.ghost.example.com".to_string(),
            auth_token: None,
            auto_sync: false,
            sync_interval_ms: 30000, // 30 seconds
        }
    }
}

/// Team/workspace information
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner_id: String,
    pub member_ids: Vec<String>,
    /// Role of each member, keyed by user id. Access control (e.g. who may
    /// add members) is decided by role, not by mere membership.
    #[serde(default)]
    pub member_roles: HashMap<String, MemberRole>,
    pub workflows: Vec<String>, // workflow IDs
    pub created_at: u64,
}

/// Team member role
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum MemberRole {
    Owner,
    Admin,
    Member,
    Viewer,
}

/// Audit log entry for enterprise compliance
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuditLog {
    pub id: String,
    pub timestamp: u64,
    pub user_id: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub details: String,
    pub ip_address: Option<String>,
}

/// Cloud sync manager
pub struct CloudSyncManager {
    // Retained for the future real backend; unused while cloud sync is a
    // disabled in-memory stub (authenticate/sync/load all return errors).
    #[allow(dead_code)]
    config: CloudConfig,
    workspaces: HashMap<String, Workspace>,
    audit_logs: Vec<AuditLog>,
}

impl CloudSyncManager {
    pub fn new(config: CloudConfig) -> Self {
        CloudSyncManager {
            config,
            workspaces: HashMap::new(),
            audit_logs: Vec::new(),
        }
    }

    /// Authenticate with cloud service
    pub fn authenticate(&mut self, _token: String) -> Result<bool, String> {
        // Cloud sync is disabled in this build - placeholder implementation
        Err("Cloud sync is not available in this build".to_string())
    }

    /// Sync workflows to cloud
    pub fn sync_workflows(&self, _workflows: &[Workflow]) -> Result<Vec<String>, String> {
        Err("Cloud sync is not available in this build".to_string())
    }

    /// Load workflows from cloud
    pub fn load_workflows(&self) -> Result<Vec<Workflow>, String> {
        Err("Cloud sync is not available in this build".to_string())
    }

    /// Create a new workspace
    pub fn create_workspace(&mut self, name: String, owner_id: String) -> Workspace {
        let id = Uuid::new_v4().to_string();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let workspace = Workspace {
            id: id.clone(),
            name,
            description: String::new(),
            owner_id: owner_id.clone(),
            member_ids: vec![owner_id.clone()],
            member_roles: HashMap::from([(owner_id.clone(), MemberRole::Owner)]),
            workflows: Vec::new(),
            created_at: now,
        };

        self.workspaces.insert(id.clone(), workspace.clone());
        self.log_audit(AuditLog {
            id: Uuid::new_v4().to_string(),
            timestamp: now,
            user_id: owner_id.clone(),
            action: "workspace_created".to_string(),
            resource_type: "workspace".to_string(),
            resource_id: id,
            details: format!("Created workspace: {}", workspace.name),
            ip_address: None,
        });

        workspace
    }

    /// Add a member to a workspace. Only members holding the Owner or Admin
    /// role may add members; Members and Viewers are rejected.
    pub fn add_member(
        &mut self,
        workspace_id: &str,
        user_id: String,
        role: MemberRole,
        requester_id: String,
    ) -> Result<(), String> {
        let workspace = self
            .workspaces
            .get_mut(workspace_id)
            .ok_or_else(|| "Workspace not found".to_string())?;

        let authorized = matches!(
            workspace.member_roles.get(&requester_id),
            Some(MemberRole::Owner) | Some(MemberRole::Admin)
        );
        if !authorized {
            return Err("Unauthorized: only workspace owners and admins can add members".into());
        }

        workspace.member_ids.push(user_id.clone());
        workspace.member_roles.insert(user_id.clone(), role.clone());

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.log_audit(AuditLog {
            id: Uuid::new_v4().to_string(),
            timestamp: now,
            user_id: requester_id,
            action: "member_added".to_string(),
            resource_type: "workspace".to_string(),
            resource_id: workspace_id.to_string(),
            details: format!("Added member {} with role {:?}", user_id, role),
            ip_address: None,
        });

        Ok(())
    }

    /// Log an audit event
    pub fn log_audit(&mut self, log: AuditLog) {
        self.audit_logs.push(log);
    }

    /// Get audit logs, newest first. `limit` caps the result to the *most
    /// recent* N entries — never the oldest, which is what a compliance view
    /// of "the last N actions" must show.
    pub fn get_audit_logs(&self, limit: Option<usize>) -> Vec<&AuditLog> {
        let newest_first = self.audit_logs.iter().rev();
        match limit {
            Some(n) => newest_first.take(n).collect(),
            None => newest_first.collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> CloudSyncManager {
        CloudSyncManager::new(CloudConfig::default())
    }

    #[test]
    fn authenticate_returns_error_in_this_build() {
        let mut mgr = manager();
        let result = mgr.authenticate("token123".to_string());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Cloud sync is not available in this build"
        );
    }

    #[test]
    fn sync_workflows_returns_error_in_this_build() {
        let mgr = manager();
        let result = mgr.sync_workflows(&[]);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Cloud sync is not available in this build"
        );
    }

    #[test]
    fn load_workflows_returns_error_in_this_build() {
        let mgr = manager();
        let result = mgr.load_workflows();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Cloud sync is not available in this build"
        );
    }

    #[test]
    fn create_workspace_adds_owner_as_member_and_logs_audit() {
        let mut mgr = manager();
        let workspace = mgr.create_workspace("Acme".to_string(), "owner-1".to_string());

        assert_eq!(workspace.owner_id, "owner-1");
        assert_eq!(workspace.member_ids, vec!["owner-1".to_string()]);
        assert!(workspace.workflows.is_empty());

        let logs = mgr.get_audit_logs(None);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action, "workspace_created");
    }

    #[test]
    fn add_member_by_owner_succeeds() {
        let mut mgr = manager();
        let workspace = mgr.create_workspace("Acme".to_string(), "owner-1".to_string());

        let result = mgr.add_member(
            &workspace.id,
            "new-user".to_string(),
            MemberRole::Member,
            "owner-1".to_string(),
        );
        assert!(result.is_ok());

        let updated = mgr.workspaces.get(&workspace.id).unwrap();
        assert!(updated.member_ids.contains(&"new-user".to_string()));
    }

    #[test]
    fn add_member_to_unknown_workspace_errors() {
        let mut mgr = manager();
        let result = mgr.add_member(
            "nonexistent",
            "new-user".to_string(),
            MemberRole::Member,
            "owner-1".to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn add_member_by_non_member_is_unauthorized() {
        let mut mgr = manager();
        let workspace = mgr.create_workspace("Acme".to_string(), "owner-1".to_string());

        let result = mgr.add_member(
            &workspace.id,
            "new-user".to_string(),
            MemberRole::Member,
            "stranger".to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn add_member_by_viewer_is_unauthorized() {
        // Authorization is role-based: a Viewer is a member but must not be
        // able to change workspace membership.
        let mut mgr = manager();
        let workspace = mgr.create_workspace("Acme".to_string(), "owner-1".to_string());

        mgr.add_member(
            &workspace.id,
            "viewer".to_string(),
            MemberRole::Viewer,
            "owner-1".to_string(),
        )
        .unwrap();

        let result = mgr.add_member(
            &workspace.id,
            "another-user".to_string(),
            MemberRole::Admin,
            "viewer".to_string(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("owners and admins"));
    }

    #[test]
    fn add_member_by_admin_succeeds_but_plain_member_is_rejected() {
        let mut mgr = manager();
        let workspace = mgr.create_workspace("Acme".to_string(), "owner-1".to_string());

        mgr.add_member(
            &workspace.id,
            "admin".to_string(),
            MemberRole::Admin,
            "owner-1".to_string(),
        )
        .unwrap();
        mgr.add_member(
            &workspace.id,
            "member".to_string(),
            MemberRole::Member,
            "owner-1".to_string(),
        )
        .unwrap();

        assert!(mgr
            .add_member(
                &workspace.id,
                "added-by-admin".to_string(),
                MemberRole::Member,
                "admin".to_string(),
            )
            .is_ok());
        assert!(mgr
            .add_member(
                &workspace.id,
                "added-by-member".to_string(),
                MemberRole::Member,
                "member".to_string(),
            )
            .is_err());
    }

    #[test]
    fn get_audit_logs_limit_returns_most_recent_entries_first() {
        // "limit" means "the most recent N", newest first — a compliance view
        // of recent activity must never silently show the oldest entries.
        let mut mgr = manager();
        mgr.create_workspace("First".to_string(), "owner-1".to_string());
        mgr.create_workspace("Second".to_string(), "owner-1".to_string());

        let limited = mgr.get_audit_logs(Some(1));
        assert_eq!(limited.len(), 1);
        assert!(limited[0].details.contains("Second"));

        let all = mgr.get_audit_logs(None);
        assert_eq!(all.len(), 2);
        assert!(all[0].details.contains("Second"));
        assert!(all[1].details.contains("First"));
    }
}
