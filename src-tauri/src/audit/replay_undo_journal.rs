//! Undo journal for routine replay runs.
//!
//! Most replayed input (clicks, scrolls, shortcuts) is not safely invertible
//! across arbitrary apps. v1 records those steps for audit only and reverses
//! **typed literal text** via synthesized backspaces when the recording retained
//! the characters.

use crate::core::events::{InputEvent, KeyAction};
use serde::{Deserialize, Serialize};

/// A single reversible (or audit-only) replay step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum ReplayUndoOp {
    /// Reverse literal typed characters replayed into the focused app.
    Backspace { count: u32 },
    /// Audit record for a click that cannot be safely inverted.
    ClickApplied { step_index: usize, x: i32, y: i32 },
    /// Audit record for non-invertible input (scroll, shortcut chord, wait).
    SkippedRevert { step_index: usize, kind: String },
}

/// Ordered journal of replay steps for WAL persistence and best-effort undo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReplayUndoJournal {
    ops: Vec<ReplayUndoOp>,
}

impl ReplayUndoJournal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_event(&mut self, step_index: usize, event: &InputEvent) {
        match event {
            InputEvent::Key { chars, action, .. }
                if matches!(action, KeyAction::Down) && !chars.is_empty() =>
            {
                let count = chars.chars().count() as u32;
                if count > 0 {
                    self.ops.push(ReplayUndoOp::Backspace { count });
                }
            }
            InputEvent::MouseClick { x, y, .. } => {
                self.ops.push(ReplayUndoOp::ClickApplied {
                    step_index,
                    x: *x,
                    y: *y,
                });
            }
            other => {
                self.ops.push(ReplayUndoOp::SkippedRevert {
                    step_index,
                    kind: replay_event_kind(other),
                });
            }
        }
    }

    pub fn ops(&self) -> &[ReplayUndoOp] {
        &self.ops
    }

    pub fn reversed(&self) -> impl Iterator<Item = &ReplayUndoOp> {
        self.ops.iter().rev()
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn reversible_backspace_count(&self) -> u32 {
        self.ops
            .iter()
            .filter_map(|op| match op {
                ReplayUndoOp::Backspace { count } => Some(*count),
                _ => None,
            })
            .sum()
    }
}

/// Snapshot persisted after each replayed event (write-ahead durability).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayRunReport {
    pub events_applied: usize,
    pub events_total: usize,
    pub undo: ReplayUndoJournal,
}

fn replay_event_kind(event: &InputEvent) -> String {
    match event {
        InputEvent::Key { .. } => "key".to_string(),
        InputEvent::Scroll { .. } => "scroll".to_string(),
        InputEvent::Delay { .. } => "delay".to_string(),
        InputEvent::Wait { .. } => "wait".to_string(),
        InputEvent::VisualCheck { .. } => "visual_check".to_string(),
        InputEvent::Variable { .. } => "variable".to_string(),
        InputEvent::VariableRef { .. } => "variable_ref".to_string(),
        InputEvent::MouseClick { .. } => "mouse_click".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::InputEvent;

    #[test]
    fn typed_chars_record_backspace_undo() {
        let mut journal = ReplayUndoJournal::new();
        journal.record_event(
            0,
            &InputEvent::Key {
                code: 0,
                chars: "hi".to_string(),
                modifiers: 0,
                action: KeyAction::Down,
                timestamp: Some(0),
                retry_count: None,
                semantic_tag: None,
            },
        );
        assert_eq!(journal.reversible_backspace_count(), 2);
        assert!(matches!(
            journal.ops()[0],
            ReplayUndoOp::Backspace { count: 2 }
        ));
    }

    #[test]
    fn click_records_audit_only_op() {
        let mut journal = ReplayUndoJournal::new();
        journal.record_event(
            1,
            &InputEvent::MouseClick {
                x: 10,
                y: 20,
                button: 0,
                element: None,
                timestamp: Some(0),
                retry_count: None,
                semantic_tag: None,
                self_heal: None,
            },
        );
        assert!(matches!(
            journal.ops()[0],
            ReplayUndoOp::ClickApplied {
                step_index: 1,
                x: 10,
                y: 20
            }
        ));
    }
}
