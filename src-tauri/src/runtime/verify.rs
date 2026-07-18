//! Per-step verification: expected vs observed.

use crate::action_plan::types::ActionKind;
use crate::runtime::capture::CapturePath;
use crate::runtime::evidence::{StepEvidence, UiResolutionStrategy};
use crate::runtime::semantic::{self, SemanticError, UiTarget};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Verified,
    Failed,
    Skipped,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepVerification {
    pub step_id: String,
    pub label: String,
    pub expected: String,
    pub observed: String,
    pub status: VerificationStatus,
    pub continue_execution: bool,
    /// How the UI target was resolved (absent for pure FS steps / old receipts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_strategy: Option<UiResolutionStrategy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ax_quality: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// Honest undo guidance when the step is not journal-reversible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempts: Option<u32>,
    /// Which capture tier fed OCR/template (absent for AX-only / old receipts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_path: Option<CapturePath>,
}

impl StepVerification {
    pub fn verify_path(step_id: &str, label: &str, path: &Path, should_exist: bool) -> Self {
        let expected = if should_exist {
            format!("{} exists", path.display())
        } else {
            format!("{} absent", path.display())
        };
        let exists = path.exists();
        let observed = if exists {
            format!("{} exists", path.display())
        } else {
            format!("{} absent", path.display())
        };
        let ok = exists == should_exist;
        Self {
            step_id: step_id.into(),
            label: label.into(),
            expected,
            observed,
            status: if ok {
                VerificationStatus::Verified
            } else {
                VerificationStatus::Failed
            },
            continue_execution: ok,
            resolution_strategy: None,
            ax_quality: None,
            fingerprint: None,
            undo_note: None,
            attempts: None,
            capture_path: None,
        }
    }

    pub fn verify_fs_applied(step_id: &str, label: &str, path: &Path) -> Self {
        Self::verify_path(step_id, label, path, true)
    }

    pub fn not_applicable(step_id: &str, label: &str, reason: &str) -> Self {
        Self {
            step_id: step_id.into(),
            label: label.into(),
            expected: reason.into(),
            observed: reason.into(),
            status: VerificationStatus::NotApplicable,
            continue_execution: true,
            resolution_strategy: None,
            ax_quality: None,
            fingerprint: None,
            undo_note: None,
            attempts: None,
            capture_path: None,
        }
    }

    pub fn skipped(step_id: &str, label: &str, reason: &str) -> Self {
        Self {
            step_id: step_id.into(),
            label: label.into(),
            expected: "execute".into(),
            observed: reason.into(),
            status: VerificationStatus::Skipped,
            continue_execution: true,
            resolution_strategy: None,
            ax_quality: None,
            fingerprint: None,
            undo_note: None,
            attempts: None,
            capture_path: None,
        }
    }

    pub fn failed(step_id: &str, label: &str, expected: &str, observed: &str) -> Self {
        Self {
            step_id: step_id.into(),
            label: label.into(),
            expected: expected.into(),
            observed: observed.into(),
            status: VerificationStatus::Failed,
            continue_execution: false,
            resolution_strategy: None,
            ax_quality: None,
            fingerprint: None,
            undo_note: None,
            attempts: None,
            capture_path: None,
        }
    }

    pub fn verified(step_id: &str, label: &str, expected: &str, observed: &str) -> Self {
        Self {
            step_id: step_id.into(),
            label: label.into(),
            expected: expected.into(),
            observed: observed.into(),
            status: VerificationStatus::Verified,
            continue_execution: true,
            resolution_strategy: None,
            ax_quality: None,
            fingerprint: None,
            undo_note: None,
            attempts: None,
            capture_path: None,
        }
    }

    /// Merge dispatch evidence onto this verification (strategy / quality / undo note).
    pub fn with_evidence(mut self, evidence: StepEvidence) -> Self {
        if self.resolution_strategy.is_none() {
            self.resolution_strategy = evidence.resolution_strategy;
        }
        if self.ax_quality.is_none() {
            self.ax_quality = evidence.ax_quality;
        }
        if self.fingerprint.is_none() {
            self.fingerprint = evidence.fingerprint;
        }
        if self.undo_note.is_none() {
            self.undo_note = evidence.undo_note;
        }
        if self.attempts.is_none() {
            self.attempts = evidence.attempts;
        }
        if self.capture_path.is_none() {
            self.capture_path = evidence.capture_path;
        }
        if let Some(detail) = evidence.detail
            && self.observed.is_empty()
        {
            self.observed = detail;
        }
        self
    }
}

fn verify_semantic_focus(step_id: &str, label: &str, target: &UiTarget) -> StepVerification {
    let expected = format!("{} {} focused", target.app, target.role);
    match semantic::verify_postcondition(target, None) {
        Ok(observed) => StepVerification::verified(step_id, label, &expected, &observed),
        Err(SemanticError::HelperUnavailable(_)) => StepVerification::not_applicable(
            step_id,
            label,
            &format!(
                "focused {} {} (UI verification best-effort)",
                target.app, target.role
            ),
        ),
        Err(e) => {
            // UI binding is best-effort (ADR-0007): record failure but do not
            // halt the plan the way a filesystem verify failure would.
            StepVerification {
                step_id: step_id.into(),
                label: label.into(),
                expected,
                observed: e.to_string(),
                status: VerificationStatus::Failed,
                continue_execution: true,
                resolution_strategy: None,
                ax_quality: None,
                fingerprint: None,
                undo_note: Some(StepEvidence::UI_UNDO_NOTE.into()),
                attempts: None,
                capture_path: None,
            }
        }
    }
}

/// True when observed field text matches the approved value.
///
/// OCR fallback observations use an `ocr:` prefix; only the payload after that
/// prefix is matched. Matching is **digit-boundary aware** so a wrong amount
/// like `112,900` cannot pass an approved `12,900` via naive substring search.
/// A trailing zero fraction (`12,900.00`) still counts as a match; a bare `ocr:`
/// prefix alone does not.
fn observed_matches_approved(observed: &str, value: &str) -> bool {
    let want = value.trim();
    if want.is_empty() {
        return observed.trim().is_empty();
    }
    let got = observed.trim();
    let payload = got.strip_prefix("ocr:").unwrap_or(got).trim();
    payload_matches_approved(payload, want)
}

fn payload_matches_approved(payload: &str, want: &str) -> bool {
    if payload == want {
        return true;
    }
    let mut start = 0;
    while start + want.len() <= payload.len() {
        let Some(rel) = payload[start..].find(want) else {
            break;
        };
        let abs = start + rel;
        let before_ok = abs == 0
            || !payload
                .as_bytes()
                .get(abs - 1)
                .is_some_and(u8::is_ascii_digit);
        let after = &payload[abs + want.len()..];
        if before_ok && after_matches_approved_suffix(after) {
            return true;
        }
        // Advance one Unicode scalar so we never slice mid-codepoint.
        start = payload[abs..]
            .chars()
            .next()
            .map(|c| abs + c.len_utf8())
            .unwrap_or(payload.len());
    }
    false
}

/// Accept end-of-string, non-digit OCR noise, or a trailing `.0+` fraction.
fn after_matches_approved_suffix(after: &str) -> bool {
    if after.is_empty() {
        return true;
    }
    if let Some(rest) = after.strip_prefix('.') {
        let zero_len = rest.chars().take_while(|c| *c == '0').count();
        if zero_len == 0 {
            return false;
        }
        let rem = &rest[zero_len..];
        return rem.is_empty() || rem.chars().next().is_some_and(|c| !c.is_ascii_digit());
    }
    after.chars().next().is_some_and(|c| !c.is_ascii_digit())
}

fn verify_semantic_set_value(
    step_id: &str,
    label: &str,
    target: &UiTarget,
    value: &str,
) -> StepVerification {
    let expected = format!("value matches {value}");
    match semantic::verify_postcondition(target, Some(value)) {
        Ok(observed) => {
            if observed_matches_approved(&observed, value) {
                StepVerification::verified(step_id, label, &expected, &observed)
            } else {
                // Product promise: observed ≠ expected halts the run.
                // ADR-0007 best-effort applies when we cannot observe — not a wrong value.
                let mut v = StepVerification::failed(step_id, label, &expected, &observed);
                v.undo_note = Some(StepEvidence::UI_UNDO_NOTE.into());
                v
            }
        }
        Err(SemanticError::HelperUnavailable(_)) => {
            StepVerification::not_applicable(step_id, label, "semantic value set dispatched")
        }
        Err(e) => {
            // Could not read the field — do not pretend the approved value landed.
            let mut v = StepVerification::failed(step_id, label, &expected, &e.to_string());
            v.undo_note = Some(StepEvidence::UI_UNDO_NOTE.into());
            v
        }
    }
}

fn verify_semantic_expected_value(
    step_id: &str,
    label: &str,
    target: &UiTarget,
    expected_value: &Option<String>,
) -> StepVerification {
    let expected = expected_value
        .clone()
        .unwrap_or_else(|| format!("{} present", target.role));
    match semantic::verify_postcondition(target, expected_value.as_deref()) {
        Ok(observed) => {
            if let Some(want) = expected_value.as_deref() {
                if observed_matches_approved(&observed, want) {
                    StepVerification::verified(step_id, label, &expected, &observed)
                } else {
                    let mut v = StepVerification::failed(step_id, label, &expected, &observed);
                    v.undo_note = Some(StepEvidence::UI_UNDO_NOTE.into());
                    v
                }
            } else {
                StepVerification::verified(step_id, label, &expected, &observed)
            }
        }
        Err(SemanticError::HelperUnavailable(_)) => StepVerification::not_applicable(
            step_id,
            label,
            "semantic verify skipped (AX helper unavailable)",
        ),
        Err(e) => StepVerification::failed(step_id, label, &expected, &e.to_string()),
    }
}

pub fn verify_after_kind(
    kind: &ActionKind,
    step_id: &str,
    label: &str,
    evidence: Option<StepEvidence>,
) -> StepVerification {
    let mut verification = match kind {
        ActionKind::CreateFolder { path } => {
            StepVerification::verify_fs_applied(step_id, label, path)
        }
        ActionKind::MoveFile { to, .. } | ActionKind::RenameFile { to, .. } => {
            StepVerification::verify_fs_applied(step_id, label, to)
        }
        // A delete is verified by absence: the source must no longer exist
        // (it now lives in the OS Trash / Recycle Bin, per the undo record).
        ActionKind::DeleteFile { path } => {
            StepVerification::verify_path(step_id, label, path, false)
        }
        ActionKind::VerifyPath { path, should_exist } => {
            StepVerification::verify_path(step_id, label, path, *should_exist)
        }
        ActionKind::OpenApplication { name } => StepVerification::not_applicable(
            step_id,
            label,
            &format!("opened {name} (UI verification best-effort)"),
        ),
        ActionKind::SemanticFocus { target } => verify_semantic_focus(step_id, label, target),
        ActionKind::SemanticSetValue { target, value } => {
            verify_semantic_set_value(step_id, label, target, value)
        }
        ActionKind::SemanticVerify {
            target,
            expected_value,
        } => verify_semantic_expected_value(step_id, label, target, expected_value),
        ActionKind::TypeText { .. } | ActionKind::Shortcut { .. } => {
            StepVerification::not_applicable(step_id, label, "UI action dispatched")
        }
        ActionKind::UiReplay { .. } => {
            StepVerification::not_applicable(step_id, label, "UI replay step completed")
        }
        ActionKind::Wait { .. } => StepVerification::not_applicable(step_id, label, "wait elapsed"),
    };
    if let Some(ev) = evidence {
        verification = verification.with_evidence(ev);
    }
    verification
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_focus_without_helper_is_not_applicable() {
        let target = UiTarget::new("TextEdit", "AXTextArea");
        let v = verify_semantic_focus("s1", "Focus", &target);
        assert_eq!(v.status, VerificationStatus::NotApplicable);
        assert!(v.continue_execution);
    }

    #[test]
    fn semantic_set_value_without_helper_is_not_applicable() {
        let target = UiTarget::new("TextEdit", "AXTextArea");
        let v = verify_semantic_set_value("s1", "Type", &target, "hello");
        assert_eq!(v.status, VerificationStatus::NotApplicable);
        assert!(v.continue_execution);
    }

    #[test]
    fn path_verify_fails_closed() {
        let missing = std::env::temp_dir().join("ghost_verify_missing_does_not_exist");
        let v = StepVerification::verify_path("s1", "check", &missing, true);
        assert_eq!(v.status, VerificationStatus::Failed);
        assert!(!v.continue_execution);
    }

    #[test]
    fn observed_mismatch_does_not_soft_pass_on_ocr_prefix() {
        assert!(!observed_matches_approved("ocr:wrong", "12,900"));
        assert!(observed_matches_approved("ocr:12,900", "12,900"));
        assert!(observed_matches_approved("12,900.00", "12,900"));
        assert!(!observed_matches_approved("12,090", "12,900"));
    }

    #[test]
    fn observed_match_rejects_digit_adjacent_false_positives() {
        // Naive `contains` would accept these — that is the money-shot bug.
        assert!(!observed_matches_approved("112,900", "12,900"));
        assert!(!observed_matches_approved("ocr:112,900", "12,900"));
        assert!(!observed_matches_approved("12,9001", "12,900"));
        assert!(!observed_matches_approved("12,900.50", "12,900"));
        // Non-digit boundaries / OCR noise around the exact amount still pass.
        assert!(observed_matches_approved("Amount: 12,900 USD", "12,900"));
        assert!(observed_matches_approved("ocr:12,900.00", "12,900"));
    }

    #[test]
    fn value_mismatch_verification_halts() {
        let mut v = StepVerification::failed("s1", "Set amount", "value matches 12,900", "12,090");
        v.undo_note = Some(StepEvidence::UI_UNDO_NOTE.into());
        assert_eq!(v.status, VerificationStatus::Failed);
        assert!(!v.continue_execution);
    }

    #[test]
    fn semantic_verify_without_helper_stays_not_applicable() {
        let target = UiTarget::new("TextEdit", "AXTextArea");
        let v =
            verify_semantic_expected_value("s1", "Verify amount", &target, &Some("12,900".into()));
        assert_eq!(v.status, VerificationStatus::NotApplicable);
        assert!(v.continue_execution);
    }
}
