//! Versioned executable representation shared by Organizer, Routines, MCP, and workflows.

use crate::core::events::InputEvent;
use crate::organizer::file_identity::FileIdentity;
use crate::policy::{Capability, PolicyDecision};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const ACTION_PLAN_VERSION: u32 = 1;

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
    OpenApplication {
        name: String,
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
                | ActionKind::RenameFile { .. } => summary.filesystem_steps += 1,
                ActionKind::VerifyPath { .. } => summary.verify_steps += 1,
                ActionKind::OpenApplication { .. }
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
