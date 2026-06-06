use serde::{Deserialize, Serialize};

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
        text: String,
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

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ElementInfo {
    pub role: String,
    pub name: String,
    pub app: String,
    pub fallback: Option<(i32, i32)>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum KeyAction {
    Down,
    Up,
}
