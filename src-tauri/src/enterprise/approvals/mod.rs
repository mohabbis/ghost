//! Approval primitives for enterprise workflow runs.
//!
//! This module is intentionally pure and command-free. It only answers whether
//! a human approval record is sufficient for a prepared plan; deterministic
//! executors still perform policy checks, audit logging, and undo journaling.

use crate::enterprise::workflows::{ApprovalDecision, ApprovalRecord, PlannedAction};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalGate {
    pub required_scope: String,
    pub required_action_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalGateResult {
    pub approved: bool,
    pub missing_action_ids: Vec<String>,
    pub reason: Option<String>,
}

impl ApprovalGate {
    pub fn for_plan(scope: impl Into<String>, actions: &[PlannedAction]) -> Self {
        Self {
            required_scope: scope.into(),
            required_action_ids: actions
                .iter()
                .filter(|action| action.requires_approval)
                .map(|action| action.action_id.clone())
                .collect(),
        }
    }

    pub fn evaluate(&self, approvals: &[ApprovalRecord]) -> ApprovalGateResult {
        if self.required_action_ids.is_empty() {
            return ApprovalGateResult {
                approved: true,
                missing_action_ids: vec![],
                reason: None,
            };
        }

        let approved_scopes: Vec<&str> = approvals
            .iter()
            .filter(|approval| approval.decision == ApprovalDecision::Approved)
            .map(|approval| approval.scope.as_str())
            .collect();

        let covers_all = approved_scopes
            .iter()
            .any(|scope| *scope == self.required_scope || *scope == "final plan");
        if covers_all {
            return ApprovalGateResult {
                approved: true,
                missing_action_ids: vec![],
                reason: None,
            };
        }

        let missing_action_ids = self
            .required_action_ids
            .iter()
            .filter(|action_id| {
                !approved_scopes
                    .iter()
                    .any(|scope| *scope == action_id.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();

        ApprovalGateResult {
            approved: missing_action_ids.is_empty(),
            missing_action_ids,
            reason: Some("required actions have not been explicitly approved".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enterprise::workflows::{ApprovalRecord, PlannedAction};
    use chrono::Utc;

    fn action(id: &str, requires_approval: bool) -> PlannedAction {
        PlannedAction {
            action_id: id.into(),
            idempotency_key: format!("run:{id}"),
            action_type: "create_output".into(),
            source: None,
            destination: None,
            requires_approval,
            risk_class: "local-mutate".into(),
        }
    }

    fn approval(scope: &str) -> ApprovalRecord {
        ApprovalRecord {
            approval_id: format!("approval-{scope}"),
            approved_by: "manager@example.test".into(),
            approved_at: Utc::now(),
            scope: scope.into(),
            decision: ApprovalDecision::Approved,
        }
    }

    #[test]
    fn final_plan_approval_covers_required_actions() {
        let gate = ApprovalGate::for_plan("closeout", &[action("a1", true), action("a2", true)]);
        assert!(gate.evaluate(&[approval("final plan")]).approved);
    }

    #[test]
    fn reports_missing_unapproved_actions() {
        let gate = ApprovalGate::for_plan("closeout", &[action("a1", true), action("a2", true)]);
        let result = gate.evaluate(&[approval("a1")]);
        assert!(!result.approved);
        assert_eq!(result.missing_action_ids, vec!["a2"]);
    }
}
