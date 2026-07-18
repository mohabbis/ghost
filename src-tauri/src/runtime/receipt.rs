//! Human-readable execution receipt built on the audit log.

use super::capture::CapturePath;
use super::evidence::UiResolutionStrategy;
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
    /// Surfaced for quick receipt scanning (also on `verification`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_strategy: Option<UiResolutionStrategy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ax_quality: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempts: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_path: Option<CapturePath>,
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
            resolution_strategy: v.resolution_strategy,
            ax_quality: v.ax_quality,
            undo_note: v.undo_note.clone(),
            attempts: v.attempts,
            capture_path: v.capture_path,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::capture::CapturePath;
    use crate::runtime::evidence::{StepEvidence, UiResolutionStrategy};
    use crate::runtime::verify::{StepVerification, VerificationStatus};

    #[test]
    fn receipt_surfaces_ui_strategy_and_honest_undo_note() {
        let v = StepVerification::verified("s1", "Focus", "focused", "ok")
            .with_evidence(StepEvidence::ocr("ocr|Save|...", "Save"));
        let report = ExecutionReport::default();
        let receipt = build_receipt("p1", "demo", None, &report, &[v], "1", "2", false, None);
        assert_eq!(
            receipt.steps[0].resolution_strategy,
            Some(UiResolutionStrategy::Ocr)
        );
        assert!(
            receipt.steps[0]
                .undo_note
                .as_deref()
                .unwrap()
                .contains("n/a")
        );
        assert_eq!(
            receipt.steps[0].verification.status,
            VerificationStatus::Verified
        );
    }

    #[test]
    fn receipt_surfaces_capture_path_from_ocr_evidence() {
        let v = StepVerification::verified("s1", "Focus", "focused", "ok").with_evidence(
            StepEvidence::ocr("ocr|Save|...", "Save").with_capture_path(CapturePath::Stream),
        );
        let report = ExecutionReport::default();
        let receipt = build_receipt("p1", "demo", None, &report, &[v], "1", "2", false, None);
        assert_eq!(receipt.steps[0].capture_path, Some(CapturePath::Stream));
        assert_eq!(
            receipt.steps[0].verification.capture_path,
            Some(CapturePath::Stream)
        );
        let json = serde_json::to_string(&receipt.steps[0]).unwrap();
        assert!(json.contains("\"capture_path\":\"stream\""));
        assert!(!json.contains("iVBORw0")); // no PNG base64
    }
}
