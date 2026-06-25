//! The undo journal: how to reverse what the executor did.
//!
//! Invariant 8 of the trust model (`CLAUDE.md`): *reversible mutations must
//! write undo data before execution*. The executor records one [`UndoOp`] in
//! this journal immediately **before** each reversible filesystem operation, so
//! a rollback is always possible even after a partial failure.
//!
//! This module is pure data — it never touches the filesystem. The reversal
//! itself is performed by `crate::organizer::undo`, which replays the journal
//! in reverse order. Keeping the record and the runner apart means the journal
//! stays a serializable, inspectable artifact.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single reversible step, expressed as the inverse of the operation that was
/// applied. Recorded before the forward operation runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum UndoOp {
    /// Reverse a folder creation by removing the folder we created. The runner
    /// removes it **only if empty** — it never recursively deletes, so user data
    /// placed inside afterward is preserved.
    RemoveFolder { path: PathBuf },
    /// Reverse a move/rename by relocating the file back to where it started.
    /// `from` is where the file now sits (the forward op's target); `to` is its
    /// original location (the forward op's source).
    Restore { from: PathBuf, to: PathBuf },
}

/// An ordered, serializable journal of reversible steps. Steps are recorded in
/// execution order; a rollback replays them in reverse (see [`reversed`]).
///
/// Serializes transparently as a JSON array so the wire shape is just the ops.
///
/// [`reversed`]: UndoJournal::reversed
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UndoJournal {
    ops: Vec<UndoOp>,
}

impl UndoJournal {
    /// A new, empty journal.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one reversible step. Called before the forward operation runs.
    pub fn record(&mut self, op: UndoOp) {
        self.ops.push(op);
    }

    /// The recorded steps, in execution order.
    pub fn ops(&self) -> &[UndoOp] {
        &self.ops
    }

    /// The steps in the order a rollback must apply them: newest first. Moves
    /// are undone before the folders that received them are removed, so a
    /// destination folder is empty by the time its `RemoveFolder` runs.
    pub fn reversed(&self) -> impl Iterator<Item = &UndoOp> {
        self.ops.iter().rev()
    }

    /// How many steps the journal holds.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether the journal is empty.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reversed_yields_newest_first() {
        let mut j = UndoJournal::new();
        j.record(UndoOp::RemoveFolder {
            path: PathBuf::from("/z/Documents"),
        });
        j.record(UndoOp::Restore {
            from: PathBuf::from("/z/Documents/a.pdf"),
            to: PathBuf::from("/z/a.pdf"),
        });
        let order: Vec<&UndoOp> = j.reversed().collect();
        // The move-back is undone first, then the folder is removed.
        assert!(matches!(order[0], UndoOp::Restore { .. }));
        assert!(matches!(order[1], UndoOp::RemoveFolder { .. }));
    }

    /// The `op` tag and snake_case names back persistence and any future IPC;
    /// pin the wire shape.
    #[test]
    fn op_serializes_with_tag() {
        assert_eq!(
            serde_json::to_value(UndoOp::RemoveFolder {
                path: PathBuf::from("/z/Documents"),
            })
            .unwrap(),
            serde_json::json!({ "op": "remove_folder", "path": "/z/Documents" })
        );
        assert_eq!(
            serde_json::to_value(UndoOp::Restore {
                from: PathBuf::from("/b"),
                to: PathBuf::from("/a"),
            })
            .unwrap(),
            serde_json::json!({ "op": "restore", "from": "/b", "to": "/a" })
        );
    }

    #[test]
    fn journal_serializes_transparently_as_array() {
        let mut j = UndoJournal::new();
        j.record(UndoOp::RemoveFolder {
            path: PathBuf::from("/z/Documents"),
        });
        let value = serde_json::to_value(&j).unwrap();
        assert!(value.is_array());
        let back: UndoJournal = serde_json::from_value(value).unwrap();
        assert_eq!(back, j);
    }
}
