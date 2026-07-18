//! Per-step timeout / retry policy for Action Plan execution.
//!
//! Retries are **not** universal: destructive and externally consequential
//! actions never auto-retry, even if a plan JSON asks for it. See
//! `docs/macos-automation-architecture.md` ("Retries are not universal").

use super::types::{
    ActionKind, ActionStep, FallbackStrategy, RetryClass, RetryPolicy, StepCondition,
};
use crate::organizer::file_identity::FileIdentity;
use crate::policy::{Capability, PolicyDecision, RiskLevel};
use crate::runtime::semantic::UiTarget;
use std::path::PathBuf;
use std::time::Duration;

impl ActionStep {
    /// Build a step with reliability defaults derived from kind/capability.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        kind: ActionKind,
        capability: Capability,
        decision: PolicyDecision,
    ) -> Self {
        let timeout_ms = default_timeout_ms(&kind);
        let retry_policy = Some(default_retry_policy(&kind, &capability));
        let preconditions = default_preconditions(&kind);
        let fallback_strategy = default_fallback_strategy(&kind);
        let risk_level = Some(risk_level_for(&capability, &decision));
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            capability,
            decision,
            rule_path: None,
            confidence: 1.0,
            reason: String::new(),
            source_identity: None,
            timeout_ms,
            retry_policy,
            preconditions,
            postconditions: Vec::new(),
            fallback_strategy,
            risk_level,
        }
    }

    pub fn with_rule_path(mut self, rule_path: Option<PathBuf>) -> Self {
        self.rule_path = rule_path;
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = reason.into();
        self
    }

    pub fn with_source_identity(mut self, identity: Option<FileIdentity>) -> Self {
        self.source_identity = identity;
        self
    }

    pub fn with_postconditions(mut self, postconditions: Vec<StepCondition>) -> Self {
        self.postconditions = postconditions;
        self
    }
}

/// Resolve the effective retry policy for a step.
///
/// `None` / missing plan fields → derive from [`ActionKind`] + [`Capability`].
/// Stored policy is still clamped by a hard safety deny for destructive kinds.
pub fn effective_retry_policy(
    kind: &ActionKind,
    capability: &Capability,
    stored: Option<&RetryPolicy>,
) -> RetryPolicy {
    let mut policy = stored
        .cloned()
        .unwrap_or_else(|| default_retry_policy(kind, capability));
    if !kind_allows_auto_retry(kind, capability) {
        policy.class = RetryClass::Denied;
        policy.max_attempts = 1;
    } else {
        policy.max_attempts = match policy.class {
            RetryClass::Denied => 1,
            RetryClass::Cautious => policy.max_attempts.clamp(1, 2),
            RetryClass::Allowed => policy.max_attempts.clamp(1, 5),
        };
    }
    policy
}

/// How many attempts the runtime may make (including the first).
pub fn effective_max_attempts(policy: &RetryPolicy) -> u32 {
    match policy.class {
        RetryClass::Denied => 1,
        RetryClass::Cautious | RetryClass::Allowed => policy.max_attempts.max(1),
    }
}

pub fn backoff_duration(policy: &RetryPolicy, attempt_index: u32) -> Duration {
    // attempt_index is 0-based after a failure; linear backoff is enough for v1.
    let ms = policy
        .backoff_ms
        .saturating_mul(u64::from(attempt_index + 1));
    Duration::from_millis(ms.clamp(50, 5_000))
}

/// Wall-clock budget for the step (dispatch + retries). `None` = no runtime deadline.
pub fn effective_timeout_ms(kind: &ActionKind, stored: Option<u64>) -> Option<u64> {
    stored.or_else(|| default_timeout_ms(kind))
}

pub fn default_retry_policy(kind: &ActionKind, capability: &Capability) -> RetryPolicy {
    if !kind_allows_auto_retry(kind, capability) {
        return RetryPolicy::denied();
    }
    match kind {
        ActionKind::SemanticFocus { .. }
        | ActionKind::SemanticVerify { .. }
        | ActionKind::OpenApplication { .. }
        | ActionKind::VerifyPath { .. }
        | ActionKind::Wait { .. } => RetryPolicy {
            class: RetryClass::Allowed,
            max_attempts: 3,
            backoff_ms: 200,
        },
        ActionKind::SemanticSetValue { .. }
        | ActionKind::CreateFolder { .. }
        | ActionKind::MoveFile { .. }
        | ActionKind::RenameFile { .. } => RetryPolicy {
            class: RetryClass::Cautious,
            max_attempts: 2,
            backoff_ms: 250,
        },
        ActionKind::TypeText { .. }
        | ActionKind::Shortcut { .. }
        | ActionKind::UiReplay { .. }
        | ActionKind::DeleteFile { .. } => RetryPolicy::denied(),
    }
}

/// Hard deny list — never auto-retry these, regardless of stored plan fields.
pub fn kind_allows_auto_retry(kind: &ActionKind, capability: &Capability) -> bool {
    if matches!(
        capability,
        Capability::DeleteFile { .. }
            | Capability::UseNetwork { .. }
            | Capability::CaptureScreen
            | Capability::GenerateWorkflowFromPrompt
            | Capability::StartRecording
    ) {
        return false;
    }
    !matches!(
        kind,
        ActionKind::DeleteFile { .. }
            | ActionKind::Shortcut { .. }
            | ActionKind::UiReplay { .. }
            | ActionKind::TypeText { .. }
    )
}

pub fn default_timeout_ms(kind: &ActionKind) -> Option<u64> {
    Some(match kind {
        ActionKind::Wait { ms } => ms.saturating_add(1_000),
        ActionKind::SemanticFocus { .. } | ActionKind::OpenApplication { .. } => 5_000,
        ActionKind::SemanticSetValue { .. } | ActionKind::SemanticVerify { .. } => 8_000,
        ActionKind::TypeText { .. } | ActionKind::Shortcut { .. } | ActionKind::UiReplay { .. } => {
            10_000
        }
        ActionKind::CreateFolder { .. }
        | ActionKind::MoveFile { .. }
        | ActionKind::RenameFile { .. }
        | ActionKind::DeleteFile { .. }
        | ActionKind::VerifyPath { .. } => 15_000,
    })
}

pub fn default_fallback_strategy(kind: &ActionKind) -> FallbackStrategy {
    match kind {
        ActionKind::SemanticFocus { .. }
        | ActionKind::SemanticSetValue { .. }
        | ActionKind::SemanticVerify { .. } => FallbackStrategy::AxThenVisionThenCoordinates,
        ActionKind::UiReplay { .. } | ActionKind::TypeText { .. } | ActionKind::Shortcut { .. } => {
            FallbackStrategy::CoordinatesOnly
        }
        _ => FallbackStrategy::None,
    }
}

pub fn risk_level_for(capability: &Capability, decision: &PolicyDecision) -> RiskLevel {
    if let PolicyDecision::RequireConfirmation { risk, .. } = decision {
        return *risk;
    }
    match capability {
        Capability::DeleteFile { .. } | Capability::UseNetwork { .. } => RiskLevel::High,
        Capability::CaptureScreen | Capability::GenerateWorkflowFromPrompt => RiskLevel::Critical,
        Capability::MoveFile { .. }
        | Capability::RenameFile { .. }
        | Capability::CopyFile { .. }
        | Capability::OsClick { .. }
        | Capability::OsType { .. }
        | Capability::OsShortcut { .. }
        | Capability::ReplayWorkflow { .. } => RiskLevel::Medium,
        Capability::CreateFolder { .. }
        | Capability::OsScroll
        | Capability::StartRecording
        | Capability::OsUnknown { .. } => RiskLevel::Medium,
        Capability::ReadFolder { .. } | Capability::OsWait => RiskLevel::Low,
    }
}

/// Default preconditions for semantic UI steps (app must be running).
pub fn default_preconditions(kind: &ActionKind) -> Vec<StepCondition> {
    match kind {
        ActionKind::SemanticFocus { target }
        | ActionKind::SemanticSetValue { target, .. }
        | ActionKind::SemanticVerify { target, .. } => {
            vec![app_running_condition(target)]
        }
        _ => Vec::new(),
    }
}

fn app_running_condition(target: &UiTarget) -> StepCondition {
    StepCondition::AppRunning {
        app: target.app.clone(),
        bundle_id: target.bundle_id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::semantic::UiTarget;
    use std::path::PathBuf;

    fn focus_kind() -> ActionKind {
        ActionKind::SemanticFocus {
            target: UiTarget::new("TextEdit", "AXTextArea"),
        }
    }

    #[test]
    fn delete_never_allows_auto_retry_even_if_plan_asks() {
        let kind = ActionKind::DeleteFile {
            path: PathBuf::from("/tmp/x"),
        };
        let cap = Capability::DeleteFile {
            path: PathBuf::from("/tmp/x"),
        };
        let asked = RetryPolicy {
            class: RetryClass::Allowed,
            max_attempts: 5,
            backoff_ms: 10,
        };
        let effective = effective_retry_policy(&kind, &cap, Some(&asked));
        assert_eq!(effective.class, RetryClass::Denied);
        assert_eq!(effective_max_attempts(&effective), 1);
    }

    #[test]
    fn shortcut_and_uireplay_deny_retry() {
        let shortcut = ActionKind::Shortcut {
            combo: "Meta+Enter".into(),
        };
        assert!(!kind_allows_auto_retry(
            &shortcut,
            &Capability::OsShortcut {
                combo: "Meta+Enter".into(),
            }
        ));
        let replay = ActionKind::UiReplay {
            events: vec![],
            step_index: 0,
        };
        assert!(!kind_allows_auto_retry(
            &replay,
            &Capability::OsClick {
                app: None,
                target_label: "btn".into(),
                low_confidence: false,
                coordinate_only: true,
            }
        ));
    }

    #[test]
    fn semantic_focus_defaults_to_allowed_retries() {
        let kind = focus_kind();
        let cap = Capability::OsClick {
            app: Some("TextEdit".into()),
            target_label: "field".into(),
            low_confidence: false,
            coordinate_only: false,
        };
        let policy = effective_retry_policy(&kind, &cap, None);
        assert_eq!(policy.class, RetryClass::Allowed);
        assert_eq!(effective_max_attempts(&policy), 3);
    }

    #[test]
    fn move_file_is_cautious() {
        let kind = ActionKind::MoveFile {
            from: PathBuf::from("/a"),
            to: PathBuf::from("/b"),
        };
        let cap = Capability::MoveFile {
            from: PathBuf::from("/a"),
            to: PathBuf::from("/b"),
        };
        let policy = effective_retry_policy(&kind, &cap, None);
        assert_eq!(policy.class, RetryClass::Cautious);
        assert_eq!(effective_max_attempts(&policy), 2);
    }

    #[test]
    fn timeout_for_wait_includes_buffer() {
        let kind = ActionKind::Wait { ms: 500 };
        assert_eq!(effective_timeout_ms(&kind, None), Some(1_500));
        assert_eq!(effective_timeout_ms(&kind, Some(9_000)), Some(9_000));
    }
}
