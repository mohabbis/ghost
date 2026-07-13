//! Human-readable execution receipt built on the audit log.

use super::verify::StepVerification;
use crate::organizer::executor::ExecutionReport;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub execution_id: Option<String>,
    pub plan_id: String,
    pub plan_title: String,
    pub started_at: String,
    pub finished_at: String,
    pub applied: usize,
    pub skipped: usize,
    pub failed: usize,
    pub stopped_early: bool,
    pub stop_reason: Option<String>,
    pub undo_available: bool,
    pub steps: Vec<ReceiptStep>,
    pub audit_event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptStep {
    pub id: String,
    pub label: String,
    pub outcome: String,
    pub verification: StepVerification,
}

#[allow(clippy::too_many_arguments)]
pub fn build_receipt(
    plan_id: &str,
    plan_title: &str,
    execution_id: Option<String>,
    report: &ExecutionReport,
    verifications: &[StepVerification],
    started_at: &str,
    finished_at: &str,
    stopped_early: bool,
    stop_reason: Option<String>,
) -> ExecutionReceipt {
    let steps: Vec<ReceiptStep> = verifications
        .iter()
        .map(|v| ReceiptStep {
            id: v.step_id.clone(),
            label: v.label.clone(),
            outcome: format!("{:?}", v.status).to_lowercase(),
            verification: v.clone(),
        })
        .collect();
    ExecutionReceipt {
        execution_id,
        plan_id: plan_id.into(),
        plan_title: plan_title.into(),
        started_at: started_at.into(),
        finished_at: finished_at.into(),
        applied: report.applied,
        skipped: report.skipped,
        failed: report.failed,
        stopped_early,
        stop_reason,
        undo_available: !report.undo.is_empty(),
        steps,
        audit_event_count: report.audit.len(),
    }
}
