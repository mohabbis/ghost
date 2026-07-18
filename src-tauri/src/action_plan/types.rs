//! Versioned executable representation shared by Organizer, Routines, MCP, and workflows.

use crate::core::events::InputEvent;
use crate::organizer::file_identity::FileIdentity;
use crate::policy::{Capability, PolicyDecision, RiskLevel};
use crate::runtime::semantic::UiTarget;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const ACTION_PLAN_VERSION: u32 = 1;

/// Whether the Action Plan runtime may auto-retry a failed step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RetryClass {
    /// Bounded retries (lookup / focus / verify style).
    Allowed,
    /// At most one careful retry (reversible mutations).
    Cautious,
    /// Never auto-retry (destructive / externally consequential / typing).
    #[default]
    Denied,
}

/// Per-step retry policy. Missing on old plans → derived at execute time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub class: RetryClass,
    /// Includes the first attempt. Clamped by class at execute time.
    pub max_attempts: u32,
    pub backoff_ms: u64,
}

impl RetryPolicy {
    pub fn denied() -> Self {
        Self {
            class: RetryClass::Denied,
            max_attempts: 1,
            backoff_ms: 0,
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::denied()
    }
}

/// How a UI step may fall back when the preferred strategy fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FallbackStrategy {
    #[default]
    None,
    AxThenVisionThenCoordinates,
    CoordinatesOnly,
}

/// Declared pre/post conditions on an [`ActionStep`].
///
/// UI semantic checks are best-effort observation, not business-effect proof
/// (ADR-0007).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum StepCondition {
    AppRunning {
        app: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bundle_id: Option<String>,
    },
    PathExists {
        path: PathBuf,
    },
    PathAbsent {
        path: PathBuf,
    },
    SemanticTarget {
        target: UiTarget,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_value: Option<String>,
    },
}

/// Where this plan originated — intent source, not execution engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PlanSource {
    Organizer {
        zone_id: String,
    },
    Routine {
        workflow_name: Option<String>,
        fingerprint: Option<String>,
    },
    Mcp {
        zone_id: String,
    },
    Workflow {
        name: String,
    },
    Demo,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionPlanSummary {
    pub total_steps: usize,
    pub filesystem_steps: usize,
    pub ui_steps: usize,
    pub verify_steps: usize,
    pub denied: usize,
    pub needs_confirmation: usize,
}

/// Human-reviewable plan: semantic steps the UI renders, not raw mouse events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlan {
    pub version: u32,
    pub id: String,
    pub title: String,
    pub source: PlanSource,
    pub steps: Vec<ActionStep>,
    pub summary: ActionPlanSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionStep {
    pub id: String,
    pub label: String,
    pub kind: ActionKind,
    pub capability: Capability,
    pub decision: PolicyDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_path: Option<PathBuf>,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_identity: Option<FileIdentity>,
    /// Wall-clock budget for dispatch + retries (ms). `None` → derive at execute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// When `None`, execute derives from kind/capability (old plans).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_policy: Option<RetryPolicy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preconditions: Vec<StepCondition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub postconditions: Vec<StepCondition>,
    #[serde(default)]
    pub fallback_strategy: FallbackStrategy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<RiskLevel>,
}

/// Executable payload for one step. Policy vocabulary lives on [`ActionStep::capability`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ActionKind {
    CreateFolder {
        path: PathBuf,
    },
    MoveFile {
        from: PathBuf,
        to: PathBuf,
    },
    RenameFile {
        from: PathBuf,
        to: PathBuf,
    },
    /// Delete a file by routing it through the OS Trash / Recycle Bin — never a
    /// direct unlink. Gated by a zone rule's `can_delete` (deny-by-default) and
    /// always High-risk confirmation; the planner never proposes this, it enters
    /// only via an explicit user action.
    DeleteFile {
        path: PathBuf,
    },
    OpenApplication {
        name: String,
    },
    /// Focus a UI element via Accessibility (macOS helper); refuses ambiguous matches.
    SemanticFocus {
        target: UiTarget,
    },
    /// Set text/value on an accessible element instead of coordinate typing.
    SemanticSetValue {
        target: UiTarget,
        value: String,
    },
    /// Post-action semantic verification (role/title/value).
    SemanticVerify {
        target: UiTarget,
        expected_value: Option<String>,
    },
    UiReplay {
        events: Vec<InputEvent>,
        step_index: usize,
    },
    TypeText {
        text: String,
        app: Option<String>,
    },
    Shortcut {
        combo: String,
    },
    Wait {
        ms: u64,
    },
    VerifyPath {
        path: PathBuf,
        should_exist: bool,
    },
}

impl ActionPlan {
    pub fn new(id: String, title: String, source: PlanSource, steps: Vec<ActionStep>) -> Self {
        let mut summary = ActionPlanSummary {
            total_steps: steps.len(),
            ..Default::default()
        };
        for step in &steps {
            match &step.decision {
                PolicyDecision::Deny { .. } => summary.denied += 1,
                PolicyDecision::RequireConfirmation { .. } => summary.needs_confirmation += 1,
                PolicyDecision::Allow => {}
            }
            match &step.kind {
                ActionKind::CreateFolder { .. }
                | ActionKind::MoveFile { .. }
                | ActionKind::RenameFile { .. }
                | ActionKind::DeleteFile { .. } => summary.filesystem_steps += 1,
                ActionKind::VerifyPath { .. } => summary.verify_steps += 1,
                ActionKind::OpenApplication { .. }
                | ActionKind::SemanticFocus { .. }
                | ActionKind::SemanticSetValue { .. }
                | ActionKind::SemanticVerify { .. }
                | ActionKind::UiReplay { .. }
                | ActionKind::TypeText { .. }
                | ActionKind::Shortcut { .. }
                | ActionKind::Wait { .. } => summary.ui_steps += 1,
            }
        }
        Self {
            version: ACTION_PLAN_VERSION,
            id,
            title,
            source,
            steps,
            summary,
        }
    }
}
