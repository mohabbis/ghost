//! Per-step execution evidence for Action Plan receipts.
//!
//! Records how a UI target was resolved (AX / OCR / template / coordinates),
//! AX quality when known, which capture path fed OCR/template when used, and
//! an honest undo note. Does **not** retain screenshots — only compact
//! metadata strings.

use serde::{Deserialize, Serialize};

use crate::runtime::capture::CapturePath;

/// How a UI step located its target at execution time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiResolutionStrategy {
    /// Accessibility tree action (`AXPress` / `AXValue`).
    Ax,
    /// ScreenCaptureKit frame + OCR text match, then coordinate click.
    Ocr,
    /// Opt-in `template_png` match, then coordinate click.
    Template,
    /// Synthetic input without AX/OCR/template (enigo type / shortcut / replay).
    Coordinates,
}

/// Compact evidence attached to a step after dispatch (no screenshot bytes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StepEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_strategy: Option<UiResolutionStrategy>,
    /// AX quality score (0–100) when the helper resolved a candidate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ax_quality: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Honest undo guidance. UI clicks/types are typically not journal-reversible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo_note: Option<String>,
    /// Attempts used including the first (from the runtime retry loop).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempts: Option<u32>,
    /// Which request-scoped capture tier produced the OCR/template frame.
    /// None for AX-only / coordinate steps that never captured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_path: Option<CapturePath>,
}

impl StepEvidence {
    pub const UI_UNDO_NOTE: &'static str =
        "n/a: UI click/type is not reversible via the undo journal";

    pub fn ui(strategy: UiResolutionStrategy) -> Self {
        Self {
            resolution_strategy: Some(strategy),
            undo_note: Some(Self::UI_UNDO_NOTE.into()),
            ..Default::default()
        }
    }

    pub fn ax(quality: impl Into<u32>, fingerprint: impl Into<String>) -> Self {
        Self {
            resolution_strategy: Some(UiResolutionStrategy::Ax),
            ax_quality: Some(quality.into()),
            fingerprint: Some(fingerprint.into()),
            undo_note: Some(Self::UI_UNDO_NOTE.into()),
            ..Default::default()
        }
    }

    pub fn ocr(fingerprint: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            resolution_strategy: Some(UiResolutionStrategy::Ocr),
            fingerprint: Some(fingerprint.into()),
            detail: Some(detail.into()),
            undo_note: Some(Self::UI_UNDO_NOTE.into()),
            ..Default::default()
        }
    }

    pub fn template(fingerprint: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            resolution_strategy: Some(UiResolutionStrategy::Template),
            fingerprint: Some(fingerprint.into()),
            detail: Some(detail.into()),
            undo_note: Some(Self::UI_UNDO_NOTE.into()),
            ..Default::default()
        }
    }

    pub fn coordinates(detail: impl Into<String>) -> Self {
        Self {
            resolution_strategy: Some(UiResolutionStrategy::Coordinates),
            detail: Some(detail.into()),
            undo_note: Some(Self::UI_UNDO_NOTE.into()),
            ..Default::default()
        }
    }

    pub fn filesystem_undo_recorded() -> Self {
        Self {
            undo_note: Some("undo journal entry recorded before mutation".into()),
            ..Default::default()
        }
    }

    pub fn with_attempts(mut self, attempts: u32) -> Self {
        self.attempts = Some(attempts.max(1));
        self
    }

    pub fn with_capture_path(mut self, path: CapturePath) -> Self {
        self.capture_path = Some(path);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_evidence_never_claims_undo() {
        let ev = StepEvidence::ui(UiResolutionStrategy::Ocr);
        assert_eq!(ev.resolution_strategy, Some(UiResolutionStrategy::Ocr));
        assert!(ev.undo_note.as_deref().unwrap().contains("n/a"));
    }

    #[test]
    fn serializes_strategy_snake_case() {
        let json = serde_json::to_string(&UiResolutionStrategy::Template).unwrap();
        assert_eq!(json, "\"template\"");
    }

    #[test]
    fn capture_path_serializes_snake_case_on_evidence() {
        let ev = StepEvidence::ocr("ocr|Save|...", "Save").with_capture_path(CapturePath::Stream);
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("\"capture_path\":\"stream\""));
        assert!(!json.contains("png"));
        assert!(!json.contains("bytes"));
    }

    #[test]
    fn ocr_evidence_can_carry_still_or_stream() {
        let still = StepEvidence::ocr("fp", "Save").with_capture_path(CapturePath::Still);
        assert_eq!(still.capture_path, Some(CapturePath::Still));
        let stream = StepEvidence::template("fp", "t").with_capture_path(CapturePath::Stream);
        assert_eq!(stream.capture_path, Some(CapturePath::Stream));
        let legacy = StepEvidence::ocr("fp", "Ok").with_capture_path(CapturePath::Legacy);
        assert_eq!(legacy.capture_path, Some(CapturePath::Legacy));
    }

    #[test]
    fn ax_only_evidence_defaults_capture_path_none() {
        let ev = StepEvidence::ax(90u32, "ax|btn");
        assert_eq!(ev.capture_path, None);
        let json = serde_json::to_string(&ev).unwrap();
        assert!(!json.contains("capture_path"));
    }

    #[test]
    fn old_receipts_without_capture_path_deserialize() {
        let json = r#"{"resolution_strategy":"ocr","fingerprint":"ocr|x","detail":"x"}"#;
        let ev: StepEvidence = serde_json::from_str(json).unwrap();
        assert_eq!(ev.capture_path, None);
        assert_eq!(ev.resolution_strategy, Some(UiResolutionStrategy::Ocr));
    }
}
