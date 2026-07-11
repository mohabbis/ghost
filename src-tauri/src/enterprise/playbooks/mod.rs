//! Human-readable playbooks that map simple trust states to strict policy.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustState {
    RunAfterReview,
    AskEveryTime,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPolicy {
    AllowAfterReview,
    AskFirst,
    Never,
}

impl From<TrustState> for ActionPolicy {
    fn from(value: TrustState) -> Self {
        match value {
            TrustState::RunAfterReview => ActionPolicy::AllowAfterReview,
            TrustState::AskEveryTime => ActionPolicy::AskFirst,
            TrustState::Never => ActionPolicy::Never,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataScope {
    pub allowed_folders: Vec<String>,
    pub allowed_file_types: Vec<String>,
    pub network_access: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybookActions {
    pub read_files: ActionPolicy,
    pub create_output: ActionPolicy,
    pub move_sources: ActionPolicy,
    pub overwrite: ActionPolicy,
    pub delete: ActionPolicy,
    pub send_email: ActionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequirements {
    pub plan_review: bool,
    pub execution_approval: bool,
    pub financial_discrepancy_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExceptionThresholds {
    pub missing_branch: String,
    pub duplicate_submission: String,
    pub total_variance_over: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub audit_days: u32,
    pub source_files_immutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnterprisePlaybook {
    pub schema_version: String,
    pub playbook_id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub data_scope: DataScope,
    pub actions: PlaybookActions,
    pub approvals: ApprovalRequirements,
    pub exceptions: ExceptionThresholds,
    pub retention: RetentionPolicy,
    pub rule_version: String,
}

impl EnterprisePlaybook {
    pub fn has_safe_financial_defaults(&self) -> bool {
        !self.data_scope.network_access
            && self.actions.overwrite == ActionPolicy::Never
            && self.actions.delete == ActionPolicy::Never
            && self.actions.send_email == ActionPolicy::Never
            && self.approvals.plan_review
            && self.approvals.execution_approval
            && self.retention.source_files_immutable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_states_map_to_strict_action_policies() {
        assert_eq!(
            ActionPolicy::from(TrustState::RunAfterReview),
            ActionPolicy::AllowAfterReview
        );
        assert_eq!(
            ActionPolicy::from(TrustState::AskEveryTime),
            ActionPolicy::AskFirst
        );
        assert_eq!(ActionPolicy::from(TrustState::Never), ActionPolicy::Never);
    }
}
