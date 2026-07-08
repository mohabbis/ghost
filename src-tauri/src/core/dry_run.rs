//! Dry-run preview: describe what a replay *would* do, without doing it.
//!
//! Pure and deterministic over the event list — no OS input is synthesized,
//! no element lookups run, nothing mutates. This backs the `dry_run_workflow`
//! command so the user can inspect "what Ghost will do next" before approving
//! a replay (trust pipeline: the plan is visible before execution).
//!
//! Typed text is never included in previews. Recordings redact secure-field
//! input at capture time, but the preview layer independently refuses to
//! surface `chars` so a preview can't become a keylogger transcript.

use crate::core::events::{ElementSelector, InputEvent, KeyAction, WaitCondition};
use serde::Serialize;

/// One event's worth of "what replay will do", aligned 1:1 with the event
/// list so step indices match live replay progress.
#[derive(Serialize, Clone, Debug)]
pub struct StepPreview {
    /// Index into the event list (matches replay progress indices).
    pub index: usize,
    /// Short action label, e.g. "Left click", "Key press", "Wait".
    pub action: String,
    /// Human-readable description of what will happen.
    pub detail: String,
    /// Recorded target element ("name — role in app"), when one was captured.
    pub target: Option<String>,
    /// Recorded screen coordinates, for events that have them.
    pub x: Option<i32>,
    pub y: Option<i32>,
    /// True when a click will run on raw coordinates with no element
    /// descriptor to re-resolve — the step most likely to break if the UI
    /// moves, flagged so the review UI can call it out.
    pub coordinate_fallback: bool,
}

/// Build the dry-run preview for a full event list.
pub fn preview_workflow(events: &[InputEvent]) -> Vec<StepPreview> {
    events
        .iter()
        .enumerate()
        .map(|(index, event)| preview_event(index, event))
        .collect()
}

fn describe_target(element: &crate::core::events::ElementInfo) -> String {
    let name = if element.name.is_empty() {
        "unnamed element"
    } else {
        &element.name
    };
    let role = if element.role.is_empty() {
        "unknown role"
    } else {
        &element.role
    };
    let base = if element.app.is_empty() || element.app == "Unknown" {
        format!("{} — {}", name, role)
    } else {
        format!("{} — {} in {}", name, role, element.app)
    };
    match &element.window_title {
        Some(title) if !title.is_empty() => format!("{} (window: {})", base, title),
        _ => base,
    }
}

fn describe_selector(selector: &ElementSelector) -> String {
    match selector {
        ElementSelector::Coordinates { x, y } => format!("point ({}, {})", x, y),
        ElementSelector::Semantic { role, name, app } => match app {
            Some(app) => format!("{} \"{}\" in {}", role, name, app),
            None => format!("{} \"{}\"", role, name),
        },
        ElementSelector::OCR { text, fuzzy } => {
            format!("on-screen text \"{}\"{}", text, if *fuzzy { " (fuzzy)" } else { "" })
        }
    }
}

fn describe_condition(condition: &WaitCondition) -> String {
    match condition {
        WaitCondition::ElementVisible { selector } => {
            format!("element visible: {}", describe_selector(selector))
        }
        WaitCondition::ElementExists { selector } => {
            format!("element exists: {}", describe_selector(selector))
        }
        WaitCondition::TextPresent { text } => format!("text present: \"{}\"", text),
        WaitCondition::ImageMatches { threshold, .. } => {
            format!("screen matches baseline (threshold {})", threshold)
        }
        WaitCondition::Custom { .. } => "custom condition".to_string(),
    }
}

fn preview_event(index: usize, event: &InputEvent) -> StepPreview {
    match event {
        InputEvent::MouseClick {
            x,
            y,
            button,
            element,
            ..
        } => {
            let action = match button {
                0 => "Left click (press)",
                1 => "Left click (release)",
                2 => "Right click (press)",
                3 => "Right click (release)",
                _ => "Click",
            };
            let is_press = matches!(button, 0 | 2);
            match element {
                // Presses re-resolve their element; releases replay at the
                // recorded point so drags end where the user ended them.
                Some(el) if is_press => StepPreview {
                    index,
                    action: action.to_string(),
                    detail: format!(
                        "Re-resolves \"{}\" and clicks its current position; falls back to \
                         the recorded point ({}, {}) if the element hasn't moved",
                        el.name, x, y
                    ),
                    target: Some(describe_target(el)),
                    x: Some(*x),
                    y: Some(*y),
                    coordinate_fallback: false,
                },
                _ if is_press => StepPreview {
                    index,
                    action: action.to_string(),
                    detail: format!(
                        "Clicks fixed coordinates ({}, {}) — no element was captured, so \
                         this step breaks if the UI moves",
                        x, y
                    ),
                    target: None,
                    x: Some(*x),
                    y: Some(*y),
                    coordinate_fallback: true,
                },
                _ => StepPreview {
                    index,
                    action: action.to_string(),
                    detail: format!("Releases the button at the recorded point ({}, {})", x, y),
                    target: None,
                    x: Some(*x),
                    y: Some(*y),
                    coordinate_fallback: false,
                },
            }
        }
        // Never surface `chars` here: previews must not reproduce typed text.
        InputEvent::Key {
            action, modifiers, ..
        } => {
            let action_label = match action {
                KeyAction::Down => "Key press",
                KeyAction::Up => "Key release",
            };
            let detail = if *modifiers != 0 {
                "Sends a recorded keystroke with modifier keys (typed text is hidden in previews)"
            } else {
                "Sends a recorded keystroke (typed text is hidden in previews)"
            };
            StepPreview {
                index,
                action: action_label.to_string(),
                detail: detail.to_string(),
                target: None,
                x: None,
                y: None,
                coordinate_fallback: false,
            }
        }
        InputEvent::Scroll { dx, dy, .. } => StepPreview {
            index,
            action: "Scroll".to_string(),
            detail: format!("Scrolls by ({}, {})", dx, dy),
            target: None,
            x: None,
            y: None,
            coordinate_fallback: false,
        },
        InputEvent::Delay { ms, .. } => StepPreview {
            index,
            action: "Wait (fixed)".to_string(),
            detail: format!("Pauses for {} ms (scaled by playback speed)", ms),
            target: None,
            x: None,
            y: None,
            coordinate_fallback: false,
        },
        InputEvent::Wait {
            condition,
            timeout_ms,
            ..
        } => StepPreview {
            index,
            action: "Wait (condition)".to_string(),
            detail: format!(
                "Waits up to {} ms for {}",
                timeout_ms,
                describe_condition(condition)
            ),
            target: None,
            x: None,
            y: None,
            coordinate_fallback: false,
        },
        InputEvent::VisualCheck { threshold, .. } => StepPreview {
            index,
            action: "Visual check".to_string(),
            detail: format!(
                "Compares the screen against a baseline screenshot (threshold {})",
                threshold
            ),
            target: None,
            x: None,
            y: None,
            coordinate_fallback: false,
        },
        InputEvent::Variable { name, .. } => StepPreview {
            index,
            action: "Set variable".to_string(),
            detail: format!("Resolves variable \"{}\"", name),
            target: None,
            x: None,
            y: None,
            coordinate_fallback: false,
        },
        InputEvent::VariableRef { name } => StepPreview {
            index,
            action: "Variable reference".to_string(),
            detail: format!("References variable \"{}\"", name),
            target: None,
            x: None,
            y: None,
            coordinate_fallback: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::ElementInfo;

    fn click(button: u8, element: Option<ElementInfo>) -> InputEvent {
        InputEvent::MouseClick {
            x: 100,
            y: 200,
            button,
            element,
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
            self_heal: None,
        }
    }

    fn element(name: &str, role: &str, app: &str) -> ElementInfo {
        ElementInfo {
            name: name.into(),
            role: role.into(),
            app: app.into(),
            ..Default::default()
        }
    }

    #[test]
    fn preview_is_one_to_one_with_events_and_mutates_nothing() {
        let events = vec![
            click(0, Some(element("Save", "AXButton", "Notes"))),
            click(1, None),
            InputEvent::Delay {
                ms: 500,
                timestamp: None,
            },
        ];
        let preview = preview_workflow(&events);
        assert_eq!(preview.len(), events.len());
        assert_eq!(preview[0].index, 0);
        assert_eq!(preview[2].index, 2);
    }

    #[test]
    fn click_with_element_describes_reresolution_not_fallback() {
        let preview = preview_workflow(&[click(0, Some(element("Save", "AXButton", "Notes")))]);
        assert_eq!(preview[0].action, "Left click (press)");
        assert!(!preview[0].coordinate_fallback);
        assert_eq!(
            preview[0].target.as_deref(),
            Some("Save — AXButton in Notes")
        );
        assert!(preview[0].detail.contains("Re-resolves"));
    }

    #[test]
    fn target_includes_window_title_when_captured() {
        let mut el = element("Save", "AXButton", "Notes");
        el.window_title = Some("Quarterly Report".to_string());
        let preview = preview_workflow(&[click(0, Some(el))]);
        assert_eq!(
            preview[0].target.as_deref(),
            Some("Save — AXButton in Notes (window: Quarterly Report)")
        );
    }

    #[test]
    fn coordinate_only_press_is_flagged_as_fallback() {
        let preview = preview_workflow(&[click(2, None)]);
        assert_eq!(preview[0].action, "Right click (press)");
        assert!(preview[0].coordinate_fallback);
        assert_eq!(preview[0].x, Some(100));
        assert_eq!(preview[0].y, Some(200));
    }

    #[test]
    fn release_is_not_flagged_as_fallback() {
        // Releases intentionally replay at recorded coordinates (drag
        // semantics); flagging them would drown real fallback risks in noise.
        let preview = preview_workflow(&[click(1, None), click(3, None)]);
        assert!(!preview[0].coordinate_fallback);
        assert!(!preview[1].coordinate_fallback);
    }

    #[test]
    fn typed_text_never_appears_in_previews() {
        let secret = "hunter2-super-secret";
        let events = vec![InputEvent::Key {
            code: 4,
            chars: secret.to_string(),
            modifiers: 0,
            action: KeyAction::Down,
            timestamp: None,
            retry_count: None,
            semantic_tag: None,
        }];
        let preview = preview_workflow(&events);
        let serialized = serde_json::to_string(&preview).unwrap();
        assert!(!serialized.contains(secret));
        assert!(preview[0].detail.contains("hidden"));
    }

    #[test]
    fn wait_condition_is_described() {
        let events = vec![InputEvent::Wait {
            condition: WaitCondition::ElementVisible {
                selector: ElementSelector::Semantic {
                    role: "AXButton".into(),
                    name: "Submit".into(),
                    app: None,
                },
            },
            timeout_ms: 3000,
            poll_interval_ms: 100,
        }];
        let preview = preview_workflow(&events);
        assert_eq!(preview[0].action, "Wait (condition)");
        assert!(preview[0].detail.contains("Submit"));
        assert!(preview[0].detail.contains("3000"));
    }
}
