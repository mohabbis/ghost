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
        ActionKind::VerifyPath { path, should_exist } => {
            StepVerification::verify_path(step_id, label, path, *should_exist)
        }
        ActionKind::OpenApplication { name } => StepVerification::not_applicable(
            step_id,
            label,
            &format!("opened {name} (UI verification best-effort)"),
        ),
        ActionKind::TypeText { .. } | ActionKind::Shortcut { .. } => {
            StepVerification::not_applicable(step_id, label, "UI action dispatched")
        }
        ActionKind::UiReplay { .. } => {
            StepVerification::not_applicable(step_id, label, "UI replay step completed")
        }
        ActionKind::Wait { .. } => StepVerification::not_applicable(step_id, label, "wait elapsed"),
    }
}
