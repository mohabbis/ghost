//! The Organizer undo runner: roll an executed plan back.
//!
//! This is the **Undo** step of the trust pipeline. It consumes the
//! [`UndoJournal`] the executor produced and replays it **in reverse order**
//! (newest step first), restoring the filesystem toward its pre-execution
//! state:
//!
//! ```text
//! Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
//!                                                              ^^^^^^
//!                                                              this module
//! ```
//!
//! Reversal preserves the same safety stance as the executor:
//!
//! - It never overwrites: a `Restore` whose origin is now occupied is skipped.
//! - It never recursively deletes: a `RemoveFolder` removes the folder **only
//!   if empty**, so files a user placed inside afterward are kept.
//! - It is partial-failure tolerant: each step is independent and the report
//!   accounts for every one (reverted, skipped, or failed).

use crate::audit::{UndoJournal, UndoOp};
use serde::{Deserialize, Serialize};
use std::fs;

/// The outcome of replaying an undo journal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndoReport {
    /// Steps that successfully reversed their operation.
    pub reverted: usize,
    /// Steps intentionally not applied (nothing to do, or an occupied origin /
    /// non-empty folder that must not be clobbered).
    pub skipped: usize,
    /// Steps that errored while reversing.
    pub failed: usize,
}

/// Replay `journal` in reverse, restoring the filesystem toward its prior state.
pub fn revert(journal: &UndoJournal) -> UndoReport {
    let mut report = UndoReport::default();

    for op in journal.reversed() {
        match op {
            UndoOp::Restore { from, to } => {
                // `from` is where the file currently sits; `to` is its original
                // home. If the file isn't where we left it, there's nothing to
                // undo; if its origin is now occupied, refuse rather than clobber.
                if !from.exists() {
                    report.skipped += 1;
                    continue;
                }
                if to.exists() {
                    report.skipped += 1;
                    continue;
                }
                if let Some(parent) = to.parent() {
                    if !parent.exists() {
                        let _ = fs::create_dir_all(parent);
                    }
                }
                match fs::rename(from, to) {
                    Ok(()) => report.reverted += 1,
                    Err(_) => report.failed += 1,
                }
            }
            UndoOp::RemoveFolder { path } => {
                if !path.exists() {
                    report.skipped += 1;
                    continue;
                }
                // `remove_dir` only succeeds on an empty directory — exactly the
                // guarantee we want. A non-empty folder (user added files after
                // execution) errors and is left in place, counted as skipped.
                match fs::remove_dir(path) {
                    Ok(()) => report.reverted += 1,
                    Err(_) => report.skipped += 1,
                }
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::executor::execute_plan;
    use crate::organizer::planner::plan_with_rules;
    use crate::organizer::testutil::{tempdir, TempDir};
    use crate::policy::FolderRule;
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

    fn fixture() -> (TempDir, Vec<FolderRule>) {
        let tmp = tempdir();
        tmp.file("report.pdf", b"a");
        tmp.file("photo.JPG", b"b");
        tmp.file("song.mp3", b"c");
        let rules = vec![full_rule(tmp.path())];
        (tmp, rules)
    }

    fn listing(root: &Path) -> Vec<String> {
        let mut out = Vec::new();
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    out.push(p.strip_prefix(base).unwrap().display().to_string());
                    if p.is_dir() {
                        walk(&p, base, out);
                    }
                }
            }
        }
        walk(root, root, &mut out);
        out.sort();
        out
    }

    #[test]
    fn undo_round_trips_an_executed_plan_back_to_the_original_tree() {
        let (tmp, rules) = fixture();
        let before = listing(tmp.path());

        let plan = plan_with_rules("z", &rules);
        let report = execute_plan(&plan, &rules);
        assert!(report.applied >= 3);
        assert_ne!(
            listing(tmp.path()),
            before,
            "execution should change the tree"
        );

        let undo = revert(&report.undo);
        assert_eq!(undo.failed, 0);
        // Files are back at the root and the created category folders are gone.
        assert_eq!(
            listing(tmp.path()),
            before,
            "undo must restore the original tree exactly"
        );
    }

    #[test]
    fn undo_keeps_a_folder_a_user_refilled_after_execution() {
        let (tmp, rules) = fixture();
        let plan = plan_with_rules("z", &rules);
        let report = execute_plan(&plan, &rules);

        // Simulate the user dropping a new file into a created category folder.
        let docs = tmp.path().join("Documents");
        std::fs::write(docs.join("user-added.txt"), b"keep me").unwrap();

        let undo = revert(&report.undo);
        // The moved file is restored, but the non-empty Documents folder is kept
        // (never recursively deleted) — the user's file survives.
        assert!(tmp.path().join("report.pdf").exists());
        assert!(docs.join("user-added.txt").exists());
        assert!(undo.skipped >= 1, "the non-empty folder removal is skipped");
    }
}
