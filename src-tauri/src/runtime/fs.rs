//! Filesystem step execution — extracted from the Organizer executor for shared runtime use.

use crate::action_plan::types::ActionKind;
use crate::audit::{UndoJournal, UndoOp};
use crate::organizer::file_identity::FileIdentity;
use crate::organizer::trash::Trasher;
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
    trasher: &dyn Trasher,
    undo: &mut UndoJournal,
) -> FsOutcome {
    let _cap = capability;
    match kind {
        ActionKind::CreateFolder { path } => apply_create_folder(path, undo),
        ActionKind::MoveFile { from, to } | ActionKind::RenameFile { from, to } => {
            let is_move = matches!(kind, ActionKind::MoveFile { .. });
            relocate(from, to, source_identity, rules, is_move, undo)
        }
        ActionKind::DeleteFile { path } => {
            delete_to_trash(path, source_identity, rules, trasher, undo)
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
        if let Some(expected) = expected_identity
            && !expected.matches(&current)
        {
            return FsOutcome::Skipped(format!(
                "source file identity changed since plan (possible TOCTOU swap): {}",
                from.display()
            ));
        }
    } else if expected_identity.is_some() {
        return FsOutcome::Skipped(format!(
            "source metadata unreadable or not a regular file: {}",
            from.display()
        ));
    }
    if let Some(parent) = to.parent()
        && !parent.exists()
    {
        return FsOutcome::Skipped(format!(
            "target parent does not exist: {}",
            parent.display()
        ));
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

/// Send `path` to the OS trash, mirroring [`relocate`]'s pre-flight stance:
/// re-check existence, refuse symlinks, re-verify file identity against the
/// plan (TOCTOU), and re-run the canonical-path policy check (`can_delete` is
/// deny-by-default). Only after the trasher reports success is
/// [`UndoOp::Untrash`] recorded — never before the mutation.
fn delete_to_trash(
    path: &Path,
    expected_identity: Option<&FileIdentity>,
    rules: &[FolderRule],
    trasher: &dyn Trasher,
    undo: &mut UndoJournal,
) -> FsOutcome {
    if !path.exists() {
        return FsOutcome::Skipped(format!("source no longer exists: {}", path.display()));
    }
    if path.is_symlink() {
        return FsOutcome::Skipped(format!(
            "source is a symlink, refusing to delete: {}",
            path.display()
        ));
    }
    if let Some(current) = FileIdentity::from_path(path) {
        if let Some(expected) = expected_identity
            && !expected.matches(&current)
        {
            return FsOutcome::Skipped(format!(
                "source file identity changed since plan (possible TOCTOU swap): {}",
                path.display()
            ));
        }
    } else if expected_identity.is_some() {
        return FsOutcome::Skipped(format!(
            "source metadata unreadable or not a regular file: {}",
            path.display()
        ));
    }
    if let Err(reason) = policy::verify_delete_at_execution(path, rules) {
        return FsOutcome::Skipped(format!("canonical path check failed: {reason}"));
    }
    match trasher.send_to_trash(path) {
        Ok(trash_ref) if !path.exists() => {
            undo.record(UndoOp::Untrash {
                original_path: path.to_path_buf(),
                trash_ref,
            });
            FsOutcome::Applied
        }
        Ok(_) => FsOutcome::Failed(format!(
            "post-delete verification failed, source still present: {}",
            path.display()
        )),
        Err(e) => FsOutcome::Failed(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::UndoJournal;
    use crate::organizer::testutil::tempdir;
    use crate::organizer::trash::test_support::RecordingTrasher;
    use crate::policy::{Capability, FolderRule};
    use std::path::Path;

    fn full_rule(path: &Path) -> FolderRule {
        FolderRule {
            path: path.to_path_buf(),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
            trust: crate::policy::TrustLevel::AskFirst,
        }
    }

    fn apply_move(
        tmp: &crate::organizer::testutil::TempDir,
        from: &Path,
        to: &Path,
        identity: Option<&FileIdentity>,
        undo: &mut UndoJournal,
    ) -> FsOutcome {
        let rules = vec![full_rule(tmp.path())];
        let kind = ActionKind::MoveFile {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
        };
        let cap = Capability::MoveFile {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
        };
        let trasher = RecordingTrasher::new(tmp.path().join(".ghost-holding"));
        apply_filesystem_step(&kind, &cap, identity, &rules, &trasher, undo)
    }

    fn apply_delete(
        tmp: &crate::organizer::testutil::TempDir,
        path: &Path,
        identity: Option<&FileIdentity>,
        can_delete: bool,
        trasher: &RecordingTrasher,
        undo: &mut UndoJournal,
    ) -> FsOutcome {
        let mut rule = full_rule(tmp.path());
        rule.can_delete = can_delete;
        let kind = ActionKind::DeleteFile {
            path: path.to_path_buf(),
        };
        let cap = Capability::DeleteFile {
            path: path.to_path_buf(),
        };
        apply_filesystem_step(&kind, &cap, identity, &[rule], trasher, undo)
    }

    #[test]
    fn never_overwrites_an_existing_target() {
        let tmp = tempdir();
        tmp.file("report.pdf", b"new");
        tmp.file("Documents/report.pdf", b"original");
        let mut undo = UndoJournal::new();
        let outcome = apply_move(
            &tmp,
            &tmp.path().join("report.pdf"),
            &tmp.path().join("Documents/report.pdf"),
            None,
            &mut undo,
        );
        assert!(matches!(outcome, FsOutcome::Skipped(_)));
        assert!(undo.is_empty());
    }

    #[test]
    fn delete_is_denied_without_can_delete_grant() {
        let tmp = tempdir();
        let f = tmp.file("junk.tmp", b"x");
        let trasher = RecordingTrasher::new(tmp.path().join(".ghost-holding"));
        let mut undo = UndoJournal::new();
        let outcome = apply_delete(&tmp, &f, None, false, &trasher, &mut undo);
        assert!(matches!(outcome, FsOutcome::Skipped(_)));
        assert!(f.exists(), "denied delete must leave the file untouched");
        assert!(trasher.trashed_paths().is_empty());
        assert!(undo.is_empty());
    }

    #[test]
    fn delete_routes_through_trash_and_records_undo_after_success() {
        let tmp = tempdir();
        let f = tmp.file("junk.tmp", b"x");
        let identity = FileIdentity::from_path(&f).expect("identity");
        let trasher = RecordingTrasher::new(tmp.path().join(".ghost-holding"));
        let mut undo = UndoJournal::new();
        let outcome = apply_delete(&tmp, &f, Some(&identity), true, &trasher, &mut undo);
        assert_eq!(outcome, FsOutcome::Applied);
        assert!(!f.exists(), "source must be gone after the delete");
        assert_eq!(trasher.trashed_paths(), vec![f.clone()]);
        assert!(matches!(
            undo.ops(),
            [UndoOp::Untrash { original_path, trash_ref }]
                if *original_path == f && trash_ref.starts_with("test-holding:")
        ));
    }

    #[test]
    fn delete_skips_identity_swap() {
        let tmp = tempdir();
        let f = tmp.file("junk.tmp", b"original");
        let expected = FileIdentity::from_path(&f).expect("identity");
        std::fs::remove_file(&f).unwrap();
        tmp.file("junk.tmp", b"swapped");
        let trasher = RecordingTrasher::new(tmp.path().join(".ghost-holding"));
        let mut undo = UndoJournal::new();
        let outcome = apply_delete(&tmp, &f, Some(&expected), true, &trasher, &mut undo);
        assert!(
            matches!(outcome, FsOutcome::Skipped(reason) if reason.contains("identity changed"))
        );
        assert!(f.exists(), "swapped file must not be deleted");
        assert!(trasher.trashed_paths().is_empty());
        assert!(undo.is_empty());
    }

    #[test]
    fn delete_refuses_symlinks() {
        let tmp = tempdir();
        let target = tmp.file("real.tmp", b"x");
        let link = tmp.path().join("link.tmp");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(windows)]
        {
            if std::os::windows::fs::symlink_file(&target, &link).is_err() {
                return; // symlink creation needs privileges on Windows; skip
            }
        }
        let trasher = RecordingTrasher::new(tmp.path().join(".ghost-holding"));
        let mut undo = UndoJournal::new();
        let outcome = apply_delete(&tmp, &link, None, true, &trasher, &mut undo);
        assert!(matches!(outcome, FsOutcome::Skipped(reason) if reason.contains("symlink")));
        assert!(target.exists());
        assert!(undo.is_empty());
    }

    #[test]
    fn relocate_skips_identity_swap() {
        let tmp = tempdir();
        let path = tmp.file("report.pdf", b"original");
        let expected = FileIdentity::from_path(&path).expect("identity");
        tmp.dir("Documents");
        std::fs::remove_file(&path).unwrap();
        tmp.file("report.pdf", b"swapped");
        let to = tmp.path().join("Documents/report.pdf");
        let mut undo = UndoJournal::new();
        let outcome = apply_move(&tmp, &path, &to, Some(&expected), &mut undo);
        assert!(
            matches!(outcome, FsOutcome::Skipped(reason) if reason.contains("identity changed"))
        );
        assert!(undo.is_empty());
    }
}
