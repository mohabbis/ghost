//! Filesystem step execution — extracted from the Organizer executor for shared runtime use.

use crate::action_plan::types::ActionKind;
use crate::audit::{UndoJournal, UndoOp};
use crate::organizer::file_identity::FileIdentity;
use crate::policy::{self, Capability, FolderRule};
use std::fs;
use std::path::Path;

/// Result of attempting one filesystem mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsOutcome {
    Applied,
    Skipped(String),
    Failed(String),
}

pub fn apply_filesystem_step(
    kind: &ActionKind,
    capability: &Capability,
    source_identity: Option<&FileIdentity>,
    rules: &[FolderRule],
    undo: &mut UndoJournal,
) -> FsOutcome {
    let _cap = capability;
    match kind {
        ActionKind::CreateFolder { path } => apply_create_folder(path, undo),
        ActionKind::MoveFile { from, to } | ActionKind::RenameFile { from, to } => {
            let is_move = matches!(kind, ActionKind::MoveFile { .. });
            relocate(from, to, source_identity, rules, is_move, undo)
        }
        _ => FsOutcome::Skipped(format!(
            "not a filesystem step for capability {capability:?}"
        )),
    }
}

fn apply_create_folder(path: &Path, undo: &mut UndoJournal) -> FsOutcome {
    if path.exists() {
        return FsOutcome::Skipped(format!("folder already exists: {}", path.display()));
    }
    let inverse = UndoOp::RemoveFolder {
        path: path.to_path_buf(),
    };
    match fs::create_dir_all(path) {
        Ok(()) if path.is_dir() => {
            undo.record(inverse);
            FsOutcome::Applied
        }
        Ok(()) => FsOutcome::Failed(format!("folder not created: {}", path.display())),
        Err(e) => FsOutcome::Failed(e.to_string()),
    }
}

fn relocate(
    from: &Path,
    to: &Path,
    expected_identity: Option<&FileIdentity>,
    rules: &[FolderRule],
    is_move: bool,
    undo: &mut UndoJournal,
) -> FsOutcome {
    if !from.exists() {
        return FsOutcome::Skipped(format!("source no longer exists: {}", from.display()));
    }
    if from.is_symlink() {
        return FsOutcome::Skipped(format!(
            "source is a symlink, refusing to move: {}",
            from.display()
        ));
    }
    if let Some(current) = FileIdentity::from_path(from) {
        if let Some(expected) = expected_identity {
            if !expected.matches(&current) {
                return FsOutcome::Skipped(format!(
                    "source file identity changed since plan (possible TOCTOU swap): {}",
                    from.display()
                ));
            }
        }
    } else if expected_identity.is_some() {
        return FsOutcome::Skipped(format!(
            "source metadata unreadable or not a regular file: {}",
            from.display()
        ));
    }
    if let Some(parent) = to.parent() {
        if !parent.exists() {
            return FsOutcome::Skipped(format!(
                "target parent does not exist: {}",
                parent.display()
            ));
        }
    }
    if let Err(reason) = policy::verify_relocate_at_execution(from, to, rules, is_move) {
        return FsOutcome::Skipped(format!("canonical path check failed: {reason}"));
    }
    if to.exists() {
        return FsOutcome::Skipped(format!(
            "target already exists, refusing to overwrite: {}",
            to.display()
        ));
    }
    let inverse = UndoOp::Restore {
        from: to.to_path_buf(),
        to: from.to_path_buf(),
    };
    match fs::rename(from, to) {
        Ok(()) if to.exists() && !from.exists() => {
            undo.record(inverse);
            FsOutcome::Applied
        }
        Ok(()) => FsOutcome::Failed(format!(
            "post-move verification failed for {}",
            to.display()
        )),
        Err(e) => FsOutcome::Failed(e.to_string()),
    }
}
