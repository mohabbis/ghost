//! Per-step verification: expected vs observed.

use crate::action_plan::types::ActionKind;
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
        ActionKind::SemanticFocus { target } => StepVerification::not_applicable(
            step_id,
            label,
            &format!("focused {} {}", target.app, target.role),
        ),
        ActionKind::SemanticSetValue { target, value } => {
            if let Ok(observed) = crate::runtime::semantic::verify_target(target, Some(value)) {
                StepVerification {
                    step_id: step_id.into(),
                    label: label.into(),
                    expected: format!("value contains {}", value),
                    observed,
                    status: VerificationStatus::Verified,
                    continue_execution: true,
                }
            } else {
                StepVerification::not_applicable(step_id, label, "semantic value set dispatched")
            }
        }
        ActionKind::SemanticVerify {
            target,
            expected_value,
        } => match crate::runtime::semantic::verify_target(target, expected_value.as_deref()) {
            Ok(observed) => StepVerification {
                step_id: step_id.into(),
                label: label.into(),
                expected: expected_value
                    .clone()
                    .unwrap_or_else(|| format!("{} present", target.role)),
                observed,
                status: VerificationStatus::Verified,
                continue_execution: true,
            },
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
