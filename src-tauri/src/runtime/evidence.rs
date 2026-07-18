//! Per-step execution evidence for Action Plan receipts.
//!
//! Records how a UI target was resolved (AX / OCR / template / coordinates),
//! AX quality when known, and an honest undo note. Does **not** retain
//! screenshots — only compact metadata strings.

use serde::{Deserialize, Serialize};

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
}
