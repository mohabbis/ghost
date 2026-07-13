//! Policy evaluation for compressed routine (replay) timelines.
//!
//! Organizer already routes every filesystem mutation through [`crate::policy::evaluate`].
//! Recorded routines historically stopped at Ghost Guard findings. This module is the
//! bridge that turns each [`CompressedStep`] into a [`Capability`] and runs it through
//! the same deny-by-default engine, producing a per-step plan the UI can approve.
//!
//! ```text
//! Compress -> Guard audit -> Policy plan (this module) -> User approval -> Replay
//! ```
//!
//! Until app/window Zones exist, OS-control capabilities never return bare `Allow`:
//! they require confirmation (or are denied). That matches AGENTS.md risk class
//! `os-control` and keeps replay interruptible and explicit.

use super::{evaluate, Capability, PolicyDecision};
use crate::core::compression::{CompressedStep, CompressionReport, Target, LOW_CONFIDENCE};
use serde::{Deserialize, Serialize};

/// One step in a routine policy plan: the capability Ghost would execute and
/// the engine's decision for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineStepDecision {
    pub step_index: usize,
    pub capability: Capability,
    pub decision: PolicyDecision,
}

/// Full policy plan for a compressed timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutinePolicyPlan {
    pub steps: Vec<RoutineStepDecision>,
    /// True when every step is either `Allow` or would be allowed after confirmation
    /// (no outright `Deny`). Useful for UI gating of "Approve & Replay".
    pub can_proceed_with_approvals: bool,
    pub denied_count: usize,
    pub confirmation_count: usize,
    pub allow_count: usize,
}

/// Map a compressed semantic step to the capability the policy engine understands.
pub fn capability_for_step(step: &CompressedStep) -> Capability {
    match step {
        CompressedStep::Click(click) => {
            let (app, target_label, coordinate_only) = match &click.target {
                Some(t) => (Some(t.app.clone()), target_label(t), false),
                None => (None, "coordinate-only click".into(), true),
            };
            Capability::OsClick {
                app,
                target_label,
                low_confidence: click.confidence < LOW_CONFIDENCE,
                coordinate_only,
            }
        }
        CompressedStep::TypeText(t) => Capability::OsType {
            app: t.target.as_ref().map(|x| x.app.clone()),
            secure_field: t.secure_field,
            redacted: t.redacted,
        },
        CompressedStep::Shortcut(s) => Capability::OsShortcut {
            combo: s.combo.clone(),
        },
        CompressedStep::Scroll(_) => Capability::OsScroll,
        CompressedStep::Wait(_) => Capability::OsWait,
        CompressedStep::Unknown(u) => Capability::OsUnknown {
            description: u.description.clone(),
        },
    }
}

/// Evaluate every compressed step against the policy engine (empty folder rules).
///
/// Folder rules do not yet govern OS automation; the engine's built-in OS-control
/// arms apply. Passing rules here keeps the signature ready for future app Zones.
pub fn evaluate_compressed(report: &CompressionReport) -> RoutinePolicyPlan {
    evaluate_compressed_with_rules(report, &[])
}

/// Same as [`evaluate_compressed`], but with explicit folder rules for future
/// hybrid plans that mix file ops and OS steps.
pub fn evaluate_compressed_with_rules(
    report: &CompressionReport,
    rules: &[crate::policy::FolderRule],
) -> RoutinePolicyPlan {
    let mut steps = Vec::with_capacity(report.steps.len());
    let mut denied_count = 0;
    let mut confirmation_count = 0;
    let mut allow_count = 0;

    for (step_index, step) in report.steps.iter().enumerate() {
        let capability = capability_for_step(step);
        let decision = evaluate(&capability, rules);
        match &decision {
            PolicyDecision::Allow => allow_count += 1,
            PolicyDecision::Deny { .. } => denied_count += 1,
            PolicyDecision::RequireConfirmation { .. } => confirmation_count += 1,
        }
        steps.push(RoutineStepDecision {
            step_index,
            capability,
            decision,
        });
    }

    RoutinePolicyPlan {
        can_proceed_with_approvals: denied_count == 0,
        denied_count,
        confirmation_count,
        allow_count,
        steps,
    }
}

fn target_label(target: &Target) -> String {
    let name = target.name.trim();
    if !name.is_empty() {
        format!("{} ({})", name, target.app)
    } else if !target.role.is_empty() {
        format!("{} in {}", target.role, target.app)
    } else {
        target.app.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::compression::{
        ClickButton, ClickStep, ShortcutStep, TypeTextStep, UnknownStep, WaitStep,
    };
    use crate::policy::RiskLevel;

    fn click(target: Option<Target>, confidence: f32) -> CompressedStep {
        let coordinate_only = target.is_none();
        CompressedStep::Click(ClickStep {
            button: ClickButton::Left,
            target,
            fallback_coords: if coordinate_only {
                Some((10, 20))
            } else {
                None
            },
            confidence,
            raw_event_count: 1,
        })
    }

    fn target(app: &str, name: &str) -> Target {
        Target {
            name: name.into(),
            role: "AXButton".into(),
            app: app.into(),
            identifier: None,
        }
    }

    #[test]
    fn wait_steps_are_allowed() {
        let plan = evaluate_compressed(&CompressionReport::new(
            1,
            vec![CompressedStep::Wait(WaitStep {
                ms: 100,
                raw_event_count: 1,
            })],
            vec![(0, 1)],
        ));
        assert_eq!(plan.allow_count, 1);
        assert!(plan.can_proceed_with_approvals);
        assert!(matches!(plan.steps[0].decision, PolicyDecision::Allow));
    }

    #[test]
    fn normal_click_requires_confirmation() {
        let plan = evaluate_compressed(&CompressionReport::new(
            1,
            vec![click(Some(target("Notes", "New")), 0.9)],
            vec![(0, 1)],
        ));
        assert_eq!(plan.confirmation_count, 1);
        assert!(plan.can_proceed_with_approvals);
        assert!(matches!(
            plan.steps[0].decision,
            PolicyDecision::RequireConfirmation {
                risk: RiskLevel::Medium,
                ..
            }
        ));
    }

    #[test]
    fn secure_field_typing_is_denied() {
        let plan = evaluate_compressed(&CompressionReport::new(
            1,
            vec![CompressedStep::TypeText(TypeTextStep {
                char_count: 8,
                redacted: true,
                secure_field: true,
                target: Some(target("Login", "Password")),
                text: None,
                confidence: 1.0,
                raw_event_count: 8,
            })],
            vec![(0, 8)],
        ));
        assert_eq!(plan.denied_count, 1);
        assert!(!plan.can_proceed_with_approvals);
    }

    #[test]
    fn unknown_steps_are_denied() {
        let plan = evaluate_compressed(&CompressionReport::new(
            1,
            vec![CompressedStep::Unknown(UnknownStep {
                description: "garbled".into(),
                raw_event_count: 1,
            })],
            vec![(0, 1)],
        ));
        assert!(plan.steps[0].decision.is_denied());
    }

    #[test]
    fn destructive_shortcut_needs_high_confirmation() {
        let plan = evaluate_compressed(&CompressionReport::new(
            1,
            vec![CompressedStep::Shortcut(ShortcutStep {
                combo: "Cmd+Shift+Delete".into(),
                action: Some("clear history".into()),
                raw_event_count: 1,
            })],
            vec![(0, 1)],
        ));
        assert!(matches!(
            plan.steps[0].decision,
            PolicyDecision::RequireConfirmation {
                risk: RiskLevel::High,
                ..
            }
        ));
    }

    #[test]
    fn coordinate_only_click_is_high_risk_confirmation() {
        let plan = evaluate_compressed(&CompressionReport::new(
            1,
            vec![click(None, 0.2)],
            vec![(0, 1)],
        ));
        assert!(matches!(
            plan.steps[0].decision,
            PolicyDecision::RequireConfirmation {
                risk: RiskLevel::High,
                ..
            }
        ));
    }
}
