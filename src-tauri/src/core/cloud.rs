//! Cloud sync capabilities for Ghost workflows.
//! Provides secure synchronization across devices.

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
    pub fn authenticate(&mut self, token: String) -> Result<bool, String> {
        // In a real implementation, this would validate the token with the API
        self.config.auth_token = Some(token);
        Ok(true)
    }

    /// Sync workflows to cloud
    pub fn sync_workflows(&self, workflows: &[Workflow]) -> Result<Vec<String>, String> {
        if self.config.auth_token.is_none() {
            return Err("Not authenticated. Please login first.".to_string());
        }

        // In a real implementation, this would make API calls
        // For now, return the workflow names as "synced"
        Ok(workflows.iter().map(|w| w.name.clone()).collect())
    }

    /// Load workflows from cloud
    pub fn load_workflows(&self) -> Result<Vec<Workflow>, String> {
        if self.config.auth_token.is_none() {
            return Err("Not authenticated. Please login first.".to_string());
        }

        // In a real implementation, this would fetch from the API
        Ok(Vec::new())
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

    /// Add a member to a workspace
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

        // Check if requester is owner or admin (simplified)
        if workspace.owner_id != requester_id && !workspace.member_ids.contains(&requester_id) {
            return Err("Unauthorized".to_string());
        }

        workspace.member_ids.push(user_id.clone());

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

    /// Get audit logs (for enterprise compliance)
    pub fn get_audit_logs(&self, limit: Option<usize>) -> Vec<&AuditLog> {
        match limit {
            Some(n) => self.audit_logs.iter().take(n).collect(),
            None => self.audit_logs.iter().collect(),
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
    fn authenticate_stores_token() {
        let mut mgr = manager();
        assert!(mgr.authenticate("token123".to_string()).unwrap());
        assert_eq!(mgr.config.auth_token, Some("token123".to_string()));
    }

    #[test]
    fn sync_workflows_requires_authentication() {
        let mgr = manager();
        let result = mgr.sync_workflows(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn sync_workflows_returns_workflow_names_when_authenticated() {
        let mut mgr = manager();
        mgr.authenticate("token123".to_string()).unwrap();

        let mut wf1 = Workflow::default();
        wf1.name = "Workflow A".to_string();
        let mut wf2 = Workflow::default();
        wf2.name = "Workflow B".to_string();

        let synced = mgr.sync_workflows(&[wf1, wf2]).unwrap();
        assert_eq!(
            synced,
            vec!["Workflow A".to_string(), "Workflow B".to_string()]
        );
    }

    #[test]
    fn load_workflows_requires_authentication() {
        let mgr = manager();
        assert!(mgr.load_workflows().is_err());
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
    fn add_member_by_viewer_currently_succeeds() {
        // FIXME: add_member's authorization check only verifies that the
        // requester is *some* existing member of the workspace - it never
        // looks at that member's MemberRole. A Viewer (who should only be
        // able to read) can add new members, same as an Owner/Admin. The
        // MemberRole enum is otherwise unused for access control.
        let mut mgr = manager();
        let workspace = mgr.create_workspace("Acme".to_string(), "owner-1".to_string());

        // "viewer" becomes a member with no role tracking, then adds someone else.
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
        assert!(result.is_ok());
    }

    #[test]
    fn get_audit_logs_limit_returns_oldest_entries_first() {
        // FIXME: "limit" reads as "most recent N" in compliance UIs, but
        // get_audit_logs(Some(n)) takes the first N entries in insertion
        // order (oldest first), not the most recent N.
        let mut mgr = manager();
        mgr.create_workspace("First".to_string(), "owner-1".to_string());
        mgr.create_workspace("Second".to_string(), "owner-1".to_string());

        let limited = mgr.get_audit_logs(Some(1));
        assert_eq!(limited.len(), 1);
        assert!(limited[0].details.contains("First"));
    }
}
