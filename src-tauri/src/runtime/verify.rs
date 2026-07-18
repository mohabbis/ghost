//! Per-step verification: expected vs observed.

use crate::action_plan::types::ActionKind;
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
        }
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
            }
        }
    }
}

fn verify_semantic_set_value(
    step_id: &str,
    label: &str,
    target: &UiTarget,
    value: &str,
) -> StepVerification {
    let expected = format!("value contains {value}");
    match semantic::verify_postcondition(target, Some(value)) {
        Ok(observed) => {
            let ok = observed.contains(value)
                || observed.trim() == value.trim()
                || observed.contains("ocr:");
            if ok {
                StepVerification::verified(step_id, label, &expected, &observed)
            } else {
                StepVerification {
                    step_id: step_id.into(),
                    label: label.into(),
                    expected,
                    observed,
                    status: VerificationStatus::Failed,
                    continue_execution: true,
                }
            }
        }
        Err(SemanticError::HelperUnavailable(_)) => {
            StepVerification::not_applicable(step_id, label, "semantic value set dispatched")
        }
        Err(e) => StepVerification {
            step_id: step_id.into(),
            label: label.into(),
            expected,
            observed: e.to_string(),
            status: VerificationStatus::Failed,
            continue_execution: true,
        },
    }
}

pub fn verify_after_kind(kind: &ActionKind, step_id: &str, label: &str) -> StepVerification {
    match kind {
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
        } => match semantic::verify_postcondition(target, expected_value.as_deref()) {
            Ok(observed) => StepVerification::verified(
                step_id,
                label,
                &expected_value
                    .clone()
                    .unwrap_or_else(|| format!("{} present", target.role)),
                &observed,
            ),
            Err(SemanticError::HelperUnavailable(_)) => StepVerification::not_applicable(
                step_id,
                label,
                "semantic verify skipped (AX helper unavailable)",
            ),
            Err(e) => StepVerification::failed(
                step_id,
                label,
                "semantic element verified",
                &e.to_string(),
            ),
        },
        ActionKind::TypeText { .. } | ActionKind::Shortcut { .. } => {
            StepVerification::not_applicable(step_id, label, "UI action dispatched")
        }
        ActionKind::UiReplay { .. } => {
            StepVerification::not_applicable(step_id, label, "UI replay step completed")
        }
        ActionKind::Wait { .. } => StepVerification::not_applicable(step_id, label, "wait elapsed"),
    }
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
}
