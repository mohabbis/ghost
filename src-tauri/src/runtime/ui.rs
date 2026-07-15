//! UI step dispatch: replay slices, typing, shortcuts, semantic AX, open app.

use crate::action_plan::types::ActionKind;
use crate::core::events::{InputEvent, ReliabilitySettings};
use crate::engine::GhostEngine;
use crate::runtime::semantic::{self, SemanticError, UiTarget};
use enigo::{Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

pub enum UiOutcome {
    Applied,
    Skipped(String),
    Failed(String),
}

pub fn dispatch_ui_step(kind: &ActionKind, engine: Option<&GhostEngine>) -> UiOutcome {
    dispatch_ui_step_with_reliability(kind, engine, None)
}

pub fn dispatch_ui_step_with_reliability(
    kind: &ActionKind,
    engine: Option<&GhostEngine>,
    reliability: Option<&ReliabilitySettings>,
) -> UiOutcome {
    match kind {
        ActionKind::OpenApplication { name } => open_application(name),
        ActionKind::SemanticFocus { target } => semantic_focus(target),
        ActionKind::SemanticSetValue { target, value } => semantic_set_value(target, value),
        ActionKind::SemanticVerify {
            target,
            expected_value,
        } => semantic_verify(target, expected_value.as_deref()),
        ActionKind::TypeText { text, .. } => type_text(text),
        ActionKind::Shortcut { combo } => send_shortcut(combo),
        ActionKind::Wait { ms } => {
            thread::sleep(Duration::from_millis(*ms));
            UiOutcome::Applied
        }
        ActionKind::UiReplay { events, .. } => {
            replay_events_with_reliability(engine, events, reliability)
        }
        _ => UiOutcome::Skipped("not a UI step".into()),
    }
}

fn semantic_focus(target: &UiTarget) -> UiOutcome {
    match semantic::focus_target(target) {
        Ok(()) => UiOutcome::Applied,
        Err(SemanticError::HelperUnavailable(msg)) => UiOutcome::Skipped(msg),
        Err(SemanticError::Ambiguous(n)) => {
            UiOutcome::Skipped(format!("refusing ambiguous focus ({n} matches)"))
        }
        Err(SemanticError::StaleTarget { expected, observed }) => UiOutcome::Skipped(format!(
            "stale target refused (expected {expected}, observed {observed})"
        )),
        Err(e) => UiOutcome::Failed(e.to_string()),
    }
}

fn semantic_set_value(target: &UiTarget, value: &str) -> UiOutcome {
    match semantic::set_target_value(target, value) {
        Ok(()) => UiOutcome::Applied,
        Err(SemanticError::HelperUnavailable(_)) => {
            // Fall back to keyboard typing when the AX helper is not present.
            type_text(value)
        }
        Err(SemanticError::Ambiguous(n)) => {
            UiOutcome::Skipped(format!("refusing ambiguous set_value ({n} matches)"))
        }
        Err(SemanticError::StaleTarget { expected, observed }) => UiOutcome::Skipped(format!(
            "stale target refused (expected {expected}, observed {observed})"
        )),
        Err(e) => UiOutcome::Failed(e.to_string()),
    }
}

fn semantic_verify(target: &UiTarget, expected_value: Option<&str>) -> UiOutcome {
    match semantic::verify_target(target, expected_value) {
        Ok(observed) => {
            if let Some(expected) = expected_value
                && observed.trim() != expected.trim()
            {
                return UiOutcome::Failed(format!(
                    "semantic verify mismatch (expected {expected}, observed {observed})"
                ));
            }
            UiOutcome::Applied
        }
        Err(SemanticError::HelperUnavailable(msg)) => UiOutcome::Skipped(msg),
        Err(SemanticError::StaleTarget { expected, observed }) => UiOutcome::Skipped(format!(
            "stale target refused (expected {expected}, observed {observed})"
        )),
        Err(e) => UiOutcome::Failed(e.to_string()),
    }
}

fn open_application(name: &str) -> UiOutcome {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        if let Err(e) = open::that_detached(name) {
            return UiOutcome::Failed(format!("open application failed: {e}"));
        }
        thread::sleep(Duration::from_millis(800));
        UiOutcome::Applied
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = name;
        UiOutcome::Skipped("open application unsupported on this platform".into())
    }
}

fn type_text(text: &str) -> UiOutcome {
    match Enigo::new(&Settings::default()) {
        Ok(mut enigo) => {
            if let Err(e) = enigo.text(text) {
                return UiOutcome::Failed(format!("type text failed: {e}"));
            }
            UiOutcome::Applied
        }
        Err(e) => UiOutcome::Failed(format!("enigo init failed: {e}")),
    }
}

fn send_shortcut(combo: &str) -> UiOutcome {
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(e) => e,
        Err(e) => return UiOutcome::Failed(format!("enigo init failed: {e}")),
    };
    let parts: Vec<&str> = combo.split('+').map(|s| s.trim()).collect();
    let mut keys = Vec::new();
    for part in parts {
        let key = match part.to_lowercase().as_str() {
            "cmd" | "command" | "meta" => Key::Meta,
            "ctrl" | "control" => Key::Control,
            "alt" | "option" => Key::Alt,
            "shift" => Key::Shift,
            "s" => Key::Unicode('s'),
            "w" => Key::Unicode('w'),
            other if other.len() == 1 => Key::Unicode(other.chars().next().unwrap()),
            other => return UiOutcome::Failed(format!("unknown shortcut key: {other}")),
        };
        keys.push(key);
    }
    if keys.is_empty() {
        return UiOutcome::Skipped("empty shortcut".into());
    }
    for key in keys.iter().take(keys.len().saturating_sub(1)) {
        if let Err(e) = enigo.key(*key, enigo::Direction::Press) {
            return UiOutcome::Failed(format!("shortcut press failed: {e}"));
        }
    }
    if let Some(last) = keys.last() {
        if let Err(e) = enigo.key(*last, enigo::Direction::Press) {
            return UiOutcome::Failed(format!("shortcut key press failed: {e}"));
        }
        if let Err(e) = enigo.key(*last, enigo::Direction::Release) {
            return UiOutcome::Failed(format!("shortcut key release failed: {e}"));
        }
    }
    for key in keys.iter().take(keys.len().saturating_sub(1)).rev() {
        if let Err(e) = enigo.key(*key, enigo::Direction::Release) {
            return UiOutcome::Failed(format!("shortcut release failed: {e}"));
        }
    }
    UiOutcome::Applied
}

fn replay_events_with_reliability(
    engine: Option<&GhostEngine>,
    events: &[InputEvent],
    reliability: Option<&ReliabilitySettings>,
) -> UiOutcome {
    let Some(engine) = engine else {
        return UiOutcome::Skipped("no replay engine available".into());
    };
    if events.is_empty() {
        return UiOutcome::Skipped("empty replay slice".into());
    }
    let result = if let Some(settings) = reliability {
        engine.replay_with_reliability(events, settings, None)
    } else {
        engine.replay(events, None)
    };
    match result {
        Ok(()) => UiOutcome::Applied,
        Err(e) => UiOutcome::Failed(e.to_string()),
    }
}
