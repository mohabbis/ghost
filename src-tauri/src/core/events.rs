use serde::{Deserialize, Serialize};

/// Shared event schema for cross-platform input events.
/// All variants are serializable for IPC transmission.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum InputEvent {
    MouseClick {
        x: i32,
        y: i32,
        button: u8,
        element: Option<ElementInfo>,
    },
    Key {
        code: u16,
        chars: String,
        modifiers: u8,
        action: KeyAction,
    },
    Scroll {
        dx: i32,
        dy: i32,
        phase: u8,
    },
    Delay {
        ms: u64,
    },
}

/// Accessibility element metadata captured during recording.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ElementInfo {
    pub role: String,
    pub name: String,
    pub app: String,
    pub fallback_coords: Option<(i32, i32)>,
}

/// Keyboard action state for key events.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum KeyAction {
    Down,
    Up,
}
