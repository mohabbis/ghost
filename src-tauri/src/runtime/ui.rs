//! UI step dispatch: replay slices, typing, shortcuts, open app.

use crate::action_plan::types::ActionKind;
use crate::core::events::InputEvent;
use crate::engine::GhostEngine;
use enigo::{Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

pub enum UiOutcome {
    Applied,
    Skipped(String),
    Failed(String),
}

pub fn dispatch_ui_step(kind: &ActionKind, engine: Option<&GhostEngine>) -> UiOutcome {
    match kind {
        ActionKind::OpenApplication { name } => open_application(name),
        ActionKind::TypeText { text, .. } => type_text(text),
        ActionKind::Shortcut { combo } => send_shortcut(combo),
        ActionKind::Wait { ms } => {
            thread::sleep(Duration::from_millis(*ms));
            UiOutcome::Applied
        }
        ActionKind::UiReplay { events, .. } => replay_events(engine, events),
        _ => UiOutcome::Skipped("not a UI step".into()),
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

fn replay_events(engine: Option<&GhostEngine>, events: &[InputEvent]) -> UiOutcome {
    let Some(engine) = engine else {
        return UiOutcome::Skipped("no replay engine available".into());
    };
    if events.is_empty() {
        return UiOutcome::Skipped("empty replay slice".into());
    }
    match engine.replay(events, None) {
        Ok(()) => UiOutcome::Applied,
        Err(e) => UiOutcome::Failed(e.to_string()),
    }
}
