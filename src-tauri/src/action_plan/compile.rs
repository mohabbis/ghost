//! Compile domain-specific plans into the shared [`ActionPlan`] IR.

use super::types::{ActionKind, ActionPlan, ActionStep, PlanSource};
use crate::core::compression::{CompressedStep, CompressionReport};
use crate::core::events::InputEvent;
use crate::organizer::planner::{OrganizerPlan, PlanAction};
use crate::policy::routines::capability_for_step;
use crate::policy::{Capability, evaluate_with_attribution};
use uuid::Uuid;

pub fn from_organizer_plan(plan: &OrganizerPlan) -> ActionPlan {
    from_organizer_plan_with_source(
        plan,
        PlanSource::Organizer {
            zone_id: plan.zone_id.clone(),
        },
    )
}

pub fn from_organizer_plan_with_source(plan: &OrganizerPlan, source: PlanSource) -> ActionPlan {
    let steps = plan
        .actions
        .iter()
        .enumerate()
        .map(|(i, action)| organizer_action_to_step(i, action))
        .collect();
    ActionPlan::new(
        Uuid::new_v4().to_string(),
        format!("Organize zone {}", plan.zone_id),
        source,
        steps,
    )
}

fn organizer_action_to_step(index: usize, action: &PlanAction) -> ActionStep {
    let kind = capability_to_kind(&action.capability);
    let label = step_label(&kind, &action.capability);
    ActionStep {
        id: format!("fs-{index}"),
        label,
        kind,
        capability: action.capability.clone(),
        decision: action.decision.clone(),
        rule_path: action.rule_path.clone(),
        confidence: action.confidence,
        reason: action.reason.clone(),
        source_identity: action.source_identity.clone(),
    }
}

pub fn from_compression_report(
    report: &CompressionReport,
    events: &[InputEvent],
    workflow_name: Option<String>,
) -> ActionPlan {
    let fingerprint = Some(crate::policy::fingerprint_events(events));
    let steps = report
        .steps
        .iter()
        .enumerate()
        .map(|(i, step)| compression_step_to_action(i, step, report, events))
        .collect();
    ActionPlan::new(
        Uuid::new_v4().to_string(),
        workflow_name
            .clone()
            .unwrap_or_else(|| "Recorded routine".into()),
        PlanSource::Routine {
            workflow_name,
            fingerprint,
        },
        steps,
    )
}

fn compression_step_to_action(
    index: usize,
    step: &CompressedStep,
    report: &CompressionReport,
    events: &[InputEvent],
) -> ActionStep {
    let capability = capability_for_step(step);
    let (kind, label) = match step {
        CompressedStep::Click(c) => {
            let target = c
                .target
                .as_ref()
                .map(|t| format!("{} in {}", t.name, t.app))
                .unwrap_or_else(|| "element".into());
            let slice = event_slice(report, index, events);
            (
                ActionKind::UiReplay {
                    events: slice,
                    step_index: index,
                },
                format!("Click {target}"),
            )
        }
        CompressedStep::TypeText(t) => {
            let label = if t.redacted || t.secure_field {
                "Type text (redacted)".into()
            } else {
                format!(
                    "Type {}",
                    t.text
                        .as_deref()
                        .map(|s| {
                            if s.len() > 40 {
                                format!("{}…", &s[..40])
                            } else {
                                s.to_string()
                            }
                        })
                        .unwrap_or_else(|| "text".into())
                )
            };
            let slice = event_slice(report, index, events);
            if t.text
                .as_ref()
                .is_some_and(|s| !s.is_empty() && !t.redacted && !t.secure_field)
            {
                (
                    ActionKind::TypeText {
                        text: t.text.clone().unwrap_or_default(),
                        app: t.target.as_ref().map(|x| x.app.clone()),
                    },
                    label,
                )
            } else {
                (
                    ActionKind::UiReplay {
                        events: slice,
                        step_index: index,
                    },
                    label,
                )
            }
        }
        CompressedStep::Shortcut(s) => {
            let slice = event_slice(report, index, events);
            let label = format!("Shortcut {}", s.action.as_deref().unwrap_or(&s.combo));
            if slice.is_empty() {
                (
                    ActionKind::Shortcut {
                        combo: s.combo.clone(),
                    },
                    label,
                )
            } else {
                (
                    ActionKind::UiReplay {
                        events: slice,
                        step_index: index,
                    },
                    label,
                )
            }
        }
        CompressedStep::Scroll(s) => {
            let slice = event_slice(report, index, events);
            (
                ActionKind::UiReplay {
                    events: slice,
                    step_index: index,
                },
                format!("Scroll {:?}", s.direction),
            )
        }
        CompressedStep::Wait(w) => (ActionKind::Wait { ms: w.ms }, format!("Wait {} ms", w.ms)),
        CompressedStep::Unknown(u) => {
            let slice = event_slice(report, index, events);
            (
                ActionKind::UiReplay {
                    events: slice,
                    step_index: index,
                },
                u.description.clone(),
            )
        }
    };
    let evaluation = evaluate_with_attribution(&capability, &[]);
    ActionStep {
        id: format!("ui-{index}"),
        label,
        kind,
        capability,
        decision: evaluation.decision,
        rule_path: evaluation.rule_path,
        confidence: step.confidence(),
        reason: String::new(),
        source_identity: None,
    }
}

fn event_slice(
    report: &CompressionReport,
    step_index: usize,
    events: &[InputEvent],
) -> Vec<InputEvent> {
    report
        .raw_spans
        .get(step_index)
        .map(|&(start, len)| events.get(start..start + len).unwrap_or(&[]).to_vec())
        .unwrap_or_default()
}

fn capability_to_kind(cap: &Capability) -> ActionKind {
    match cap {
        Capability::CreateFolder { path } => ActionKind::CreateFolder { path: path.clone() },
        Capability::MoveFile { from, to } => ActionKind::MoveFile {
            from: from.clone(),
            to: to.clone(),
        },
        Capability::RenameFile { from, to } => ActionKind::RenameFile {
            from: from.clone(),
            to: to.clone(),
        },
        other => ActionKind::VerifyPath {
            path: std::path::PathBuf::from(format!("{other:?}")),
            should_exist: false,
        },
    }
}

fn step_label(kind: &ActionKind, cap: &Capability) -> String {
    match kind {
        ActionKind::CreateFolder { path } => format!("Create folder {}", path.display()),
        ActionKind::MoveFile { from, to } => {
            format!("Move {} → {}", from.display(), to.display())
        }
        ActionKind::RenameFile { from, to } => {
            format!(
                "Rename {} → {}",
                from.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| from.display().to_string()),
                to.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| to.display().to_string())
            )
        }
        ActionKind::OpenApplication { name } => format!("Open {name}"),
        ActionKind::SemanticFocus { target } => {
            format!("Focus {} {}", target.app, target.role)
        }
        ActionKind::SemanticSetValue { target, .. } => {
            format!("Set value on {} {}", target.app, target.role)
        }
        ActionKind::SemanticVerify { target, .. } => {
            format!("Verify {} {}", target.app, target.role)
        }
        ActionKind::TypeText { text, .. } => {
            if text.len() > 48 {
                format!("Type {}…", &text[..48])
            } else {
                format!("Type {text}")
            }
        }
        ActionKind::Shortcut { combo } => format!("Shortcut {combo}"),
        ActionKind::Wait { ms } => format!("Wait {ms} ms"),
        ActionKind::VerifyPath { path, should_exist } => {
            format!(
                "Verify {} {}",
                path.display(),
                if *should_exist { "exists" } else { "absent" }
            )
        }
        ActionKind::UiReplay { .. } => match cap {
            Capability::OsClick { target_label, .. } => format!("Click {target_label}"),
            Capability::OsType { .. } => "Type text".into(),
            Capability::OsScroll => "Scroll".into(),
            Capability::OsUnknown { description } => description.clone(),
            _ => "UI action".into(),
        },
    }
}
