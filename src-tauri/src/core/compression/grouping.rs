//! Low-level classification helpers used by the compressor.

use super::types::{ClickButton, ScrollDirection, ScrollMagnitude};

pub const MOD_SHIFT: u8 = 0x01;
pub const MOD_CONTROL: u8 = 0x02;
pub const MOD_ALT: u8 = 0x04;
pub const MOD_COMMAND: u8 = 0x08;

pub const MIN_MEANINGFUL_WAIT_MS: u64 = 250;

const SCROLL_MEDIUM_THRESHOLD: i64 = 120;
const SCROLL_LARGE_THRESHOLD: i64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseKind {
    Press(ClickButton),
    Release(ClickButton),
}

pub fn mouse_kind(button: u8) -> Option<MouseKind> {
    match button {
        0 => Some(MouseKind::Press(ClickButton::Left)),
        1 => Some(MouseKind::Release(ClickButton::Left)),
        2 => Some(MouseKind::Press(ClickButton::Right)),
        3 => Some(MouseKind::Release(ClickButton::Right)),
        _ => None,
    }
}

pub fn is_typed_char(chars: &str, modifiers: u8) -> bool {
    if modifiers & (MOD_COMMAND | MOD_CONTROL | MOD_ALT) != 0 {
        return false;
    }
    let mut it = chars.chars();
    match (it.next(), it.next()) {
        (Some(c), None) => !c.is_control(),
        _ => false,
    }
}

pub fn is_shortcut(chars: &str, modifiers: u8) -> bool {
    modifiers & (MOD_COMMAND | MOD_CONTROL) != 0 && !chars.is_empty()
}

pub fn combo_label(chars: &str, modifiers: u8) -> String {
    let mut parts: Vec<String> = Vec::new();
    if modifiers & MOD_COMMAND != 0 {
        parts.push("Cmd".to_string());
    }
    if modifiers & MOD_CONTROL != 0 {
        parts.push("Ctrl".to_string());
    }
    if modifiers & MOD_ALT != 0 {
        parts.push("Alt".to_string());
    }
    if modifiers & MOD_SHIFT != 0 {
        parts.push("Shift".to_string());
    }
    let key = chars.trim().to_uppercase();
    parts.push(if key.is_empty() { "?".to_string() } else { key });
    parts.join("+")
}

pub fn known_action(chars: &str, modifiers: u8) -> Option<&'static str> {
    if modifiers & (MOD_COMMAND | MOD_CONTROL) == 0 {
        return None;
    }
    match chars.trim().to_ascii_lowercase().as_str() {
        "s" => Some("Save"),
        "c" => Some("Copy"),
        "v" => Some("Paste"),
        "x" => Some("Cut"),
        "z" => Some("Undo"),
        "a" => Some("Select All"),
        "f" => Some("Find"),
        "p" => Some("Print"),
        "n" => Some("New"),
        "o" => Some("Open"),
        "w" => Some("Close"),
        "t" => Some("New Tab"),
        "r" => Some("Reload"),
        "q" => Some("Quit"),
        _ => None,
    }
}

pub fn scroll_bucket(total_dx: i64, total_dy: i64) -> (ScrollDirection, ScrollMagnitude) {
    let direction = if total_dy.abs() >= total_dx.abs() {
        if total_dy < 0 {
            ScrollDirection::Up
        } else {
            ScrollDirection::Down
        }
    } else if total_dx < 0 {
        ScrollDirection::Left
    } else {
        ScrollDirection::Right
    };

    let dominant = total_dx.abs().max(total_dy.abs());
    let magnitude = if dominant >= SCROLL_LARGE_THRESHOLD {
        ScrollMagnitude::Large
    } else if dominant >= SCROLL_MEDIUM_THRESHOLD {
        ScrollMagnitude::Medium
    } else {
        ScrollMagnitude::Small
    };

    (direction, magnitude)
}
