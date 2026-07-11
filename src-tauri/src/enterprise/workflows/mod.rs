//! Deterministic enterprise workflow run state.

use crate::policy::decision::PolicyDecision;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Explicit lifecycle states for enterprise workflow runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunState {
    Draft,
    Planning,
    AwaitingApproval,
    Approved,
    Executing,
    Paused,
    AwaitingReview,
    Completed,
    PartiallyCompleted,
    Failed,
    Cancelled,
    RolledBack,
}

/// User-selected or approved input to a workflow run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputArtifact {
    pub artifact_id: String,
    pub path: String,
    pub artifact_type: String,
    pub content_hash: Option<String>,
    pub immutable_source_copy: bool,
}

/// A deterministic action proposed before execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedAction {
    pub action_id: String,
    pub idempotency_key: String,
    pub action_type: String,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub requires_approval: bool,
    pub risk_class: String,
}

/// Approval captured before mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub approval_id: String,
    pub approved_by: String,
    pub approved_at: DateTime<Utc>,
    pub scope: String,
    pub decision: ApprovalDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    Rejected,
    NeedsChanges,
}

/// Result of one deterministic execution step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionResult {
    pub action_id: String,
    pub idempotency_key: String,
    pub status: ActionStatus,
    pub message: String,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    SkippedAlreadyApplied,
    Succeeded,
    Failed,
    AwaitingManualIntervention,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowException {
    pub exception_id: String,
    pub kind: String,
    pub severity: ExceptionSeverity,
    pub message: String,
    pub reviewer_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExceptionSeverity {
    Info,
    Review,
    Blocker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub entry_id: String,
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    pub action: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndoManifest {
    pub manifest_id: String,
    pub created_at: DateTime<Utc>,
    pub reversible: bool,
    pub entries: Vec<String>,
}

/// Complete enterprise workflow run envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnterpriseWorkflowRun {
    pub run_id: String,
    pub playbook_id: String,
    pub playbook_version: String,
    pub initiated_by: String,
    pub initiated_at: DateTime<Utc>,
    pub state: WorkflowRunState,
    pub input_manifest: Vec<InputArtifact>,
    pub proposed_actions: Vec<PlannedAction>,
    pub policy_results: Vec<PolicyDecision>,
    pub approvals: Vec<ApprovalRecord>,
    pub execution_results: Vec<ActionResult>,
    pub exceptions: Vec<WorkflowException>,
    pub audit_entries: Vec<AuditEntry>,
    pub undo_manifest: Option<UndoManifest>,
}

impl EnterpriseWorkflowRun {
    /// Mutating execution cannot start until the run is approved and all
    /// proposed actions have idempotency keys.
    pub fn can_execute(&self) -> bool {
        self.state == WorkflowRunState::Approved
            && !self.proposed_actions.is_empty()
            && self
                .proposed_actions
                .iter()
                .all(|action| !action.idempotency_key.trim().is_empty())
            && self.policy_results.iter().all(PolicyDecision::is_allowed)
            && self
                .approvals
                .iter()
                .any(|approval| approval.decision == ApprovalDecision::Approved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_run() -> EnterpriseWorkflowRun {
        EnterpriseWorkflowRun {
            run_id: "run-1".into(),
            playbook_id: "daily-branch-reporting".into(),
            playbook_version: "1.0.0".into(),
            initiated_by: "employee@example.test".into(),
            initiated_at: Utc::now(),
            state: WorkflowRunState::Approved,
            input_manifest: vec![],
            proposed_actions: vec![PlannedAction {
                action_id: "action-1".into(),
                idempotency_key: "run-1:action-1".into(),
                action_type: "create_output".into(),
                source: None,
                destination: Some("/approved/output.csv".into()),
                requires_approval: true,
                risk_class: "local-mutate".into(),
            }],
            policy_results: vec![PolicyDecision::Allow],
            approvals: vec![ApprovalRecord {
                approval_id: "approval-1".into(),
                approved_by: "manager@example.test".into(),
                approved_at: Utc::now(),
                scope: "final plan".into(),
                decision: ApprovalDecision::Approved,
            }],
            execution_results: vec![],
            exceptions: vec![],
            audit_entries: vec![],
            undo_manifest: None,
        }
    }

    #[test]
    fn approved_run_with_idempotent_actions_can_execute() {
        assert!(base_run().can_execute());
    }

    #[test]
    fn run_with_confirmation_requirement_cannot_execute() {
        let mut run = base_run();
        run.policy_results = vec![PolicyDecision::RequireConfirmation {
            reason: "manager review required".into(),
            risk: crate::policy::risk::RiskLevel::High,
        }];
        assert!(!run.can_execute());
    }

    #[test]
    fn run_without_idempotency_key_cannot_execute() {
        let mut run = base_run();
        run.proposed_actions[0].idempotency_key.clear();
        assert!(!run.can_execute());
    }
}
