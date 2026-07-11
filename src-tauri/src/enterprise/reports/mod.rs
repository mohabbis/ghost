//! Audit report summaries for enterprise workflow runs.

use crate::enterprise::workflows::{ActionStatus, EnterpriseWorkflowRun, ExceptionSeverity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunAuditSummary {
    pub run_id: String,
    pub state: String,
    pub proposed_actions: usize,
    pub succeeded_actions: usize,
    pub failed_actions: usize,
    pub blocker_exceptions: usize,
    pub audit_entries: usize,
    pub undo_available: bool,
}

impl From<&EnterpriseWorkflowRun> for RunAuditSummary {
    fn from(run: &EnterpriseWorkflowRun) -> Self {
        Self {
            run_id: run.run_id.clone(),
            state: format!("{:?}", run.state),
            proposed_actions: run.proposed_actions.len(),
            succeeded_actions: run
                .execution_results
                .iter()
                .filter(|result| result.status == ActionStatus::Succeeded)
                .count(),
            failed_actions: run
                .execution_results
                .iter()
                .filter(|result| result.status == ActionStatus::Failed)
                .count(),
            blocker_exceptions: run
                .exceptions
                .iter()
                .filter(|exception| exception.severity == ExceptionSeverity::Blocker)
                .count(),
            audit_entries: run.audit_entries.len(),
            undo_available: run
                .undo_manifest
                .as_ref()
                .is_some_and(|manifest| manifest.reversible),
        }
    }
}
