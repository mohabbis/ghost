//! Compile domain-specific plans into the shared [`ActionPlan`] IR.

use super::types::{ActionKind, ActionPlan, ActionStep, PlanSource};
use crate::core::compression::{
    ClickButton, ClickStep, CompressedStep, CompressionReport, Target, TypeTextStep,
};
use crate::core::events::InputEvent;
use crate::organizer::planner::{OrganizerPlan, PlanAction};
use crate::policy::routines::capability_for_step;
use crate::policy::{Capability, evaluate_with_attribution};
use crate::runtime::semantic::UiTarget;
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
    ActionStep::new(
        format!("fs-{index}"),
        label,
        kind,
        action.capability.clone(),
        action.decision.clone(),
    )
    .with_rule_path(action.rule_path.clone())
    .with_confidence(action.confidence)
    .with_reason(action.reason.clone())
    .with_source_identity(action.source_identity.clone())
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
        CompressedStep::Click(c) => compile_click(c, index, report, events),
        CompressedStep::TypeText(t) => compile_type_text(t, index, report, events),
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
    ActionStep::new(
        format!("ui-{index}"),
        label,
        kind,
        capability,
        evaluation.decision,
    )
    .with_rule_path(evaluation.rule_path)
    .with_confidence(step.confidence())
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

/// Prefer AX SemanticFocus when the click carries a reviewable locator;
/// otherwise keep coordinate UiReplay (right-click, weak descriptors, etc.).
fn compile_click(
    c: &ClickStep,
    index: usize,
    report: &CompressionReport,
    events: &[InputEvent],
) -> (ActionKind, String) {
    let label_target = c
        .target
        .as_ref()
        .map(|t| format!("{} in {}", t.name, t.app))
        .unwrap_or_else(|| "element".into());
    let label = format!("Click {label_target}");
    let slice = event_slice(report, index, events);

    if c.button == ClickButton::Left
        && let Some(t) = c.target.as_ref()
        && t.is_semantically_addressable(c.confidence)
    {
        let mut ui = compression_target_to_ui(t);
        if let Some(png) = template_png_from_events(&slice) {
            ui = ui.with_template_png(png);
        }
        return (ActionKind::SemanticFocus { target: ui }, label);
    }

    (
        ActionKind::UiReplay {
            events: slice,
            step_index: index,
        },
        label,
    )
}

/// Prefer semantic set_value when typed text is retained and the field has a
/// strong locator. Secure / redacted typing never takes this path (no value to
/// set; never re-surface secrets into the plan IR).
fn compile_type_text(
    t: &TypeTextStep,
    index: usize,
    report: &CompressionReport,
    events: &[InputEvent],
) -> (ActionKind, String) {
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
    let retained = t
        .text
        .as_ref()
        .is_some_and(|s| !s.is_empty() && !t.redacted && !t.secure_field);

    if retained
        && let Some(target) = t.target.as_ref()
        && target.is_semantically_addressable(t.confidence)
    {
        let mut ui = compression_target_to_ui(target);
        if let Some(png) = template_png_from_events(&slice) {
            ui = ui.with_template_png(png);
        }
        return (
            ActionKind::SemanticSetValue {
                target: ui,
                value: t.text.clone().unwrap_or_default(),
            },
            label,
        );
    }

    if retained {
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

fn compression_target_to_ui(t: &Target) -> UiTarget {
    let mut ui = UiTarget::new(t.app.clone(), t.role.clone());
    let name = t.name.trim();
    if !name.is_empty() {
        ui.title = Some(name.to_string());
    }
    ui.identifier = t.identifier.clone();
    ui.window_title = t.window_title.clone();
    ui
}

fn template_png_from_events(events: &[InputEvent]) -> Option<Box<[u8]>> {
    events.iter().find_map(|e| match e {
        InputEvent::MouseClick {
            element: Some(el), ..
        } => el.template_png.clone(),
        _ => None,
    })
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
        // Only reachable when a covering rule grants `can_delete`: the planner
        // never proposes deletes, and policy/engine.rs denies the capability
        // everywhere else. Execution routes through the OS trash (runtime/fs.rs).
        Capability::DeleteFile { path } => ActionKind::DeleteFile { path: path.clone() },
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
        ActionKind::DeleteFile { path } => {
            format!("Move {} to the OS Trash", path.display())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::compression::{
        ClickButton, ClickStep, CompressedStep, CompressionReport, Target, TypeTextStep, WaitStep,
    };
    use crate::core::events::{ElementInfo, InputEvent};

    fn report(steps: Vec<CompressedStep>, spans: Vec<(usize, usize)>) -> CompressionReport {
        CompressionReport::new(spans.iter().map(|(_, l)| l).sum(), steps, spans)
    }

    fn addressable_target() -> Target {
        Target {
            name: "Revenue".into(),
            role: "AXTextField".into(),
            app: "Numbers".into(),
            identifier: Some("cell-B7".into()),
            window_title: Some("Q3 close".into()),
        }
    }

    #[test]
    fn addressable_left_click_compiles_to_semantic_focus() {
        let steps = vec![CompressedStep::Click(ClickStep {
            button: ClickButton::Left,
            target: Some(addressable_target()),
            fallback_coords: Some((10, 20)),
            confidence: 0.95,
            raw_event_count: 1,
        })];
        let plan = from_compression_report(&report(steps, vec![(0, 0)]), &[], None);
        assert!(matches!(
            plan.steps[0].kind,
            ActionKind::SemanticFocus { .. }
        ));
        if let ActionKind::SemanticFocus { target } = &plan.steps[0].kind {
            assert_eq!(target.app, "Numbers");
            assert_eq!(target.role, "AXTextField");
            assert_eq!(target.title.as_deref(), Some("Revenue"));
            assert_eq!(target.identifier.as_deref(), Some("cell-B7"));
            assert_eq!(target.window_title.as_deref(), Some("Q3 close"));
            // Hard non-goal: no silent template capture into Action Plans.
            assert!(
                target.template_png.is_none(),
                "SemanticFocus must not invent template_png when events lack templates"
            );
            assert_eq!(
                plan.steps[0].fallback_strategy,
                crate::action_plan::FallbackStrategy::AxThenVisionThenCoordinates
            );
        }
    }

    /// Compiling addressable clicks must not attach `template_png` unless the
    /// raw events already carry an opt-in fragment (capture_element_templates).
    #[test]
    fn semantic_focus_omits_template_png_when_events_lack_templates() {
        let events = vec![InputEvent::MouseClick {
            x: 10,
            y: 20,
            button: 0,
            element: Some(ElementInfo {
                role: "AXTextField".into(),
                name: "Revenue".into(),
                app: "Numbers".into(),
                identifier: Some("cell-B7".into()),
                window_title: Some("Q3 close".into()),
                template_png: None,
                ..Default::default()
            }),
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
            self_heal: None,
        }];
        let steps = vec![CompressedStep::Click(ClickStep {
            button: ClickButton::Left,
            target: Some(addressable_target()),
            fallback_coords: Some((10, 20)),
            confidence: 0.95,
            raw_event_count: 1,
        })];
        let plan = from_compression_report(&report(steps, vec![(0, 1)]), &events, None);
        match &plan.steps[0].kind {
            ActionKind::SemanticFocus { target } => {
                assert!(
                    target.template_png.is_none(),
                    "must not auto-attach template_png from template-less events"
                );
            }
            other => panic!("expected SemanticFocus, got {other:?}"),
        }
    }

    #[test]
    fn weak_or_right_click_stays_ui_replay() {
        let weak = CompressedStep::Click(ClickStep {
            button: ClickButton::Left,
            target: Some(Target {
                name: String::new(),
                role: "AXUnknown".into(),
                app: "App".into(),
                identifier: None,
                window_title: None,
            }),
            fallback_coords: Some((1, 2)),
            confidence: 0.4,
            raw_event_count: 1,
        });
        let right = CompressedStep::Click(ClickStep {
            button: ClickButton::Right,
            target: Some(addressable_target()),
            fallback_coords: Some((1, 2)),
            confidence: 0.95,
            raw_event_count: 1,
        });
        let plan =
            from_compression_report(&report(vec![weak, right], vec![(0, 0), (0, 0)]), &[], None);
        assert!(matches!(plan.steps[0].kind, ActionKind::UiReplay { .. }));
        assert!(matches!(plan.steps[1].kind, ActionKind::UiReplay { .. }));
    }

    #[test]
    fn retained_type_with_locator_compiles_to_semantic_set_value() {
        let steps = vec![CompressedStep::TypeText(TypeTextStep {
            char_count: 8,
            redacted: false,
            secure_field: false,
            target: Some(addressable_target()),
            text: Some("48210.00".into()),
            confidence: 0.9,
            raw_event_count: 8,
        })];
        let plan = from_compression_report(&report(steps, vec![(0, 0)]), &[], None);
        match &plan.steps[0].kind {
            ActionKind::SemanticSetValue { target, value } => {
                assert_eq!(value, "48210.00");
                assert_eq!(target.identifier.as_deref(), Some("cell-B7"));
            }
            other => panic!("expected SemanticSetValue, got {other:?}"),
        }
    }

    #[test]
    fn redacted_or_secure_typing_never_becomes_semantic_set_value() {
        let redacted = CompressedStep::TypeText(TypeTextStep {
            char_count: 4,
            redacted: true,
            secure_field: false,
            target: Some(addressable_target()),
            text: None,
            confidence: 0.9,
            raw_event_count: 4,
        });
        let secure = CompressedStep::TypeText(TypeTextStep {
            char_count: 8,
            redacted: true,
            secure_field: true,
            target: Some(addressable_target()),
            text: None,
            confidence: 0.9,
            raw_event_count: 8,
        });
        let plan = from_compression_report(
            &report(vec![redacted, secure], vec![(0, 0), (0, 0)]),
            &[],
            None,
        );
        assert!(matches!(plan.steps[0].kind, ActionKind::UiReplay { .. }));
        assert!(matches!(plan.steps[1].kind, ActionKind::UiReplay { .. }));
    }

    #[test]
    fn template_png_from_click_events_attaches_to_semantic_focus() {
        let png = b"png-bytes".to_vec().into_boxed_slice();
        let events = vec![InputEvent::MouseClick {
            x: 1,
            y: 2,
            button: 0,
            element: Some(ElementInfo {
                role: "AXButton".into(),
                name: "Save".into(),
                app: "Notes".into(),
                identifier: Some("save".into()),
                window_title: Some("Untitled".into()),
                template_png: Some(png.clone()),
                ..Default::default()
            }),
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
            self_heal: None,
        }];
        let steps = vec![CompressedStep::Click(ClickStep {
            button: ClickButton::Left,
            target: Some(Target {
                name: "Save".into(),
                role: "AXButton".into(),
                app: "Notes".into(),
                identifier: Some("save".into()),
                window_title: Some("Untitled".into()),
            }),
            fallback_coords: Some((1, 2)),
            confidence: 0.95,
            raw_event_count: 1,
        })];
        let plan = from_compression_report(&report(steps, vec![(0, 1)]), &events, None);
        match &plan.steps[0].kind {
            ActionKind::SemanticFocus { target } => {
                assert_eq!(target.template_png.as_deref(), Some(png.as_ref()));
            }
            other => panic!("expected SemanticFocus, got {other:?}"),
        }
    }

    #[test]
    fn wait_steps_unchanged() {
        let plan = from_compression_report(
            &report(
                vec![CompressedStep::Wait(WaitStep {
                    ms: 300,
                    raw_event_count: 1,
                })],
                vec![(0, 1)],
            ),
            &[],
            Some("demo".into()),
        );
        assert!(matches!(plan.steps[0].kind, ActionKind::Wait { ms: 300 }));
    }

    #[test]
    fn legacy_target_json_without_window_title_deserializes() {
        let json = r#"{"name":"Ok","role":"AXButton","app":"App","identifier":null}"#;
        let t: Target = serde_json::from_str(json).unwrap();
        assert!(t.window_title.is_none());
        assert!(t.is_semantically_addressable(0.8));
    }
}
