//! Best-effort reversal of a persisted replay undo journal.
//!
//! v1 only synthesizes backspaces for literal typed text. Clicks and other
//! OS-control steps are reported as skipped — Ghost does not pretend it can
//! invert arbitrary UI state.

use crate::audit::replay_undo_journal::{ReplayUndoJournal, ReplayUndoOp};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayUndoStepOutcome {
    pub op: ReplayUndoOp,
    pub applied: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayUndoReport {
    pub reversed: usize,
    pub skipped: usize,
    pub steps: Vec<ReplayUndoStepOutcome>,
}

/// Replay the journal in reverse. Typed text is undone via backspace; everything
/// else is audit-only and counted as skipped.
pub fn revert_replay(journal: &ReplayUndoJournal) -> anyhow::Result<ReplayUndoReport> {
    let mut enigo = Enigo::new(&Settings::default())?;
    let mut report = ReplayUndoReport {
        reversed: 0,
        skipped: 0,
        steps: Vec::new(),
    };

    for op in journal.reversed() {
        match op {
            ReplayUndoOp::Backspace { count } => {
                for _ in 0..*count {
                    enigo.key(Key::Backspace, Direction::Click)?;
                }
                report.reversed += 1;
                report.steps.push(ReplayUndoStepOutcome {
                    op: op.clone(),
                    applied: true,
                    detail: format!("sent {count} backspace(s)"),
                });
            }
            ReplayUndoOp::ClickApplied { .. } | ReplayUndoOp::SkippedRevert { .. } => {
                report.skipped += 1;
                report.steps.push(ReplayUndoStepOutcome {
                    op: op.clone(),
                    applied: false,
                    detail: "not safely reversible".to_string(),
                });
            }
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::replay_undo_journal::ReplayUndoJournal;
    use crate::core::events::InputEvent;

    #[test]
    fn revert_empty_journal_is_noop() {
        let report = revert_replay(&ReplayUndoJournal::new()).unwrap();
        assert_eq!(report.reversed, 0);
        assert_eq!(report.skipped, 0);
    }

    #[test]
    fn revert_skips_non_reversible_ops() {
        let mut journal = ReplayUndoJournal::new();
        journal.record_event(
            0,
            &InputEvent::MouseClick {
                x: 1,
                y: 2,
                button: 0,
                element: None,
                timestamp: Some(0),
                retry_count: None,
                semantic_tag: None,
                self_heal: None,
            },
        );
        let report = revert_replay(&journal).unwrap();
        assert_eq!(report.reversed, 0);
        assert_eq!(report.skipped, 1);
    }
}
