//! UI step dispatch: replay slices, typing, shortcuts, semantic AX, open app.

use crate::action_plan::types::ActionKind;
use crate::core::events::{InputEvent, ReliabilitySettings};
use crate::engine::GhostEngine;
use crate::runtime::evidence::StepEvidence;
use crate::runtime::helper_budget::HelperBudget;
use crate::runtime::semantic::{self, SemanticError, UiTarget};
use enigo::{Enigo, Key, Keyboard, Settings};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// `Enigo::new` can block indefinitely on Windows CI runners with no interactive
/// input session (observed: `demo_workflow_serializes_and_runs_fs_steps` hung
/// until the 45m job timeout). Bound init on Windows only — macOS `Enigo` is
/// `!Send` (holds `CGEventSource`), so a timeout thread cannot compile there.
#[cfg(windows)]
const ENIGO_INIT_TIMEOUT: Duration = Duration::from_secs(3);
const OPEN_APP_TIMEOUT: Duration = Duration::from_secs(5);

fn enigo_or_err() -> Result<Enigo, String> {
    #[cfg(windows)]
    {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(Enigo::new(&Settings::default()));
        });
        return match rx.recv_timeout(ENIGO_INIT_TIMEOUT) {
            Ok(Ok(enigo)) => Ok(enigo),
            Ok(Err(e)) => Err(format!("enigo init failed: {e}")),
            Err(_) => {
                Err("enigo init timed out — no interactive input session (headless CI?)".into())
            }
        };
    }
    #[cfg(not(windows))]
    {
        Enigo::new(&Settings::default()).map_err(|e| format!("enigo init failed: {e}"))
    }
}

pub enum UiOutcome {
    Applied(Option<StepEvidence>),
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
    dispatch_ui_step_with_budget(kind, engine, reliability, None)
}

/// Like [`dispatch_ui_step_with_reliability`], plumbing a remaining step budget
/// into AX / ScreenCaptureKit helper calls for cooperative mid-op timeouts.
pub fn dispatch_ui_step_with_budget(
    kind: &ActionKind,
    engine: Option<&GhostEngine>,
    reliability: Option<&ReliabilitySettings>,
    budget: Option<HelperBudget>,
) -> UiOutcome {
    match kind {
        ActionKind::OpenApplication { name } => open_application(name),
        ActionKind::SemanticFocus { target } => semantic_focus(target, budget),
        ActionKind::SemanticSetValue { target, value } => semantic_set_value(target, value, budget),
        ActionKind::SemanticVerify {
            target,
            expected_value,
        } => semantic_verify(target, expected_value.as_deref()),
        ActionKind::TypeText { text, .. } => type_text(text),
        ActionKind::Shortcut { combo } => send_shortcut(combo),
        ActionKind::Wait { ms } => {
            thread::sleep(Duration::from_millis(*ms));
            UiOutcome::Applied(Some(StepEvidence::coordinates(format!("wait {ms} ms"))))
        }
        ActionKind::UiReplay { events, .. } => {
            replay_events_with_reliability(engine, events, reliability)
        }
        _ => UiOutcome::Skipped("not a UI step".into()),
    }
}

fn semantic_focus(target: &UiTarget, budget: Option<HelperBudget>) -> UiOutcome {
    match semantic::focus_target_with_budget(target, budget) {
        Ok(evidence) => UiOutcome::Applied(Some(evidence)),
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

fn semantic_set_value(target: &UiTarget, value: &str, budget: Option<HelperBudget>) -> UiOutcome {
    match semantic::set_target_value_with_budget(target, value, budget) {
        Ok(evidence) => UiOutcome::Applied(Some(evidence)),
        Err(SemanticError::HelperUnavailable(msg)) => {
            // Enigo fallback only makes sense on a real macOS GUI session where
            // the AX helper binary is missing. Off macOS (Linux/Windows CI) it
            // either types into nothing or hangs in Enigo::new — skip instead.
            #[cfg(target_os = "macos")]
            {
                let _ = msg;
                match type_text(value) {
                    UiOutcome::Applied(Some(mut ev)) => {
                        ev.detail = Some("typed via enigo; AX helper unavailable".into());
                        UiOutcome::Applied(Some(ev))
                    }
                    other => other,
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                UiOutcome::Skipped(format!(
                    "semantic set_value skipped ({msg}); enigo fallback is macOS-only"
                ))
            }
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
            UiOutcome::Applied(Some(StepEvidence::ax(0u32, observed)))
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
        // ShellExecute / xdg-open can block when the name is a macOS-only app
        // (e.g. demo "TextEdit") on Windows CI. Bound the launch call.
        let name_owned = name.to_string();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(open::that_detached(&name_owned));
        });
        match rx.recv_timeout(OPEN_APP_TIMEOUT) {
            Ok(Ok(())) => {
                thread::sleep(Duration::from_millis(800));
                UiOutcome::Applied(Some(StepEvidence::coordinates(format!("opened {name}"))))
            }
            Ok(Err(e)) => UiOutcome::Failed(format!("open application failed: {e}")),
            Err(_) => UiOutcome::Failed(format!(
                "open application timed out after {} ms ({name})",
                OPEN_APP_TIMEOUT.as_millis()
            )),
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = name;
        UiOutcome::Skipped("open application unsupported on this platform".into())
    }
}

fn type_text(text: &str) -> UiOutcome {
    let mut enigo = match enigo_or_err() {
        Ok(e) => e,
        Err(e) => return UiOutcome::Failed(e),
    };
    if let Err(e) = enigo.text(text) {
        return UiOutcome::Failed(format!("type text failed: {e}"));
    }
    UiOutcome::Applied(Some(StepEvidence::coordinates("typed via enigo")))
}

fn send_shortcut(combo: &str) -> UiOutcome {
    let mut enigo = match enigo_or_err() {
        Ok(e) => e,
        Err(e) => return UiOutcome::Failed(e),
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
    UiOutcome::Applied(Some(StepEvidence::coordinates(format!("shortcut {combo}"))))
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
        Ok(()) => UiOutcome::Applied(Some(StepEvidence::coordinates("ui replay slice"))),
        Err(e) => UiOutcome::Failed(e.to_string()),
    }
}
