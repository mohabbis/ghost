//! The Organizer executor: safely apply an approved plan.
//!
//! This is the **Execution** step of the trust pipeline — the first organizer
//! code that mutates the filesystem. It deliberately mirrors the planner's
//! caution:
//!
//! ```text
//! Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
//!                                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^
//!                                         this module
//! ```
//!
//! Given an already-approved [`OrganizerPlan`] and the Zone's [`FolderRule`]s,
//! [`execute_plan`] walks each action and, for every one:
//!
//! 1. **Re-checks policy.** Anything the engine now refuses is skipped, never
//!    applied — even though it was in the plan (rules or state may have drifted).
//! 2. **Verifies state.** The source must still exist; the target must *not*
//!    (Ghost never silently overwrites — `AGENTS.md` non-negotiable rule).
//! 3. **Writes undo before mutating.** A reversible op records its inverse in
//!    the [`UndoJournal`] *before* the filesystem call (trust invariant 8).
//! 4. **Applies, then verifies the result**, and records one audit event.
//!
//! It only ever applies the file-organization capabilities the planner emits
//! (`CreateFolder`, `MoveFile`, `RenameFile`). It never deletes, never copies
//! over an existing file, and a failure on one action leaves every other file
//! recoverable — partial progress is captured in the audit log and reversible
//! through the journal.

use crate::audit::{ActionOutcome, AuditLog, UndoJournal, UndoOp};
use crate::policy::{self, Capability, FolderRule, PolicyDecision};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use super::planner::OrganizerPlan;

/// The outcome of executing a plan: counts plus the full audit log and the undo
/// journal needed to roll the changes back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReport {
    /// Actions that ran and were verified.
    pub applied: usize,
    /// Actions intentionally not run (policy denial, missing source, occupied
    /// target, unsupported capability).
    pub skipped: usize,
    /// Actions attempted that errored.
    pub failed: usize,
    /// The full, ordered record of what happened.
    pub audit: AuditLog,
    /// The reverse operations, in execution order, for [`super::undo::revert`].
    pub undo: UndoJournal,
}

/// Apply an approved plan against the active folder rules.
///
/// The caller is responsible for passing a plan the user approved; this
/// function independently re-checks every action through the policy engine and
/// refuses anything denied, so an unapproved or stale action can never slip
/// through to the filesystem.
pub fn execute_plan(plan: &OrganizerPlan, rules: &[FolderRule]) -> ExecutionReport {
    let mut report = ExecutionReport::default();

    for action in &plan.actions {
        let cap = &action.capability;

        // (1) Re-check policy at execution time. Deny => skip, audited, no IO.
        if let PolicyDecision::Deny { reason } = policy::evaluate(cap, rules) {
            report.record_skip(cap.clone(), format!("policy denied: {reason}"));
            continue;
        }

        match apply_one(cap, &mut report.undo) {
            Outcome::Applied => {
                report.applied += 1;
                report.audit.record(cap.clone(), ActionOutcome::Applied);
            }
            Outcome::Skipped(reason) => report.record_skip(cap.clone(), reason),
            Outcome::Failed(error) => {
                report.failed += 1;
                report
                    .audit
                    .record(cap.clone(), ActionOutcome::Failed { error });
            }
        }
    }

    report
}

impl ExecutionReport {
    fn record_skip(&mut self, cap: Capability, reason: String) {
        self.skipped += 1;
        self.audit.record(cap, ActionOutcome::Skipped { reason });
    }
}

/// The result of attempting a single capability.
enum Outcome {
    Applied,
    Skipped(String),
    Failed(String),
}

/// Apply one capability, recording its undo step before any mutation. Only the
/// organizer's own file-organization capabilities are executable here; anything
/// else is skipped rather than risked.
fn apply_one(cap: &Capability, undo: &mut UndoJournal) -> Outcome {
    match cap {
        Capability::CreateFolder { path } => {
            if path.exists() {
                // Idempotent: the folder already exists (perhaps created earlier
                // in this same run). Not an error, but nothing to undo either.
                return Outcome::Skipped(format!("folder already exists: {}", path.display()));
            }
            // (3) Undo before mutating: remove the folder we are about to create.
            undo.record(UndoOp::RemoveFolder { path: path.clone() });
            match fs::create_dir_all(path) {
                Ok(()) if path.is_dir() => Outcome::Applied,
                Ok(()) => Outcome::Failed(format!("folder not created: {}", path.display())),
                Err(e) => Outcome::Failed(e.to_string()),
            }
        }
        Capability::MoveFile { from, to } | Capability::RenameFile { from, to } => {
            relocate(from, to, undo)
        }
        other => Outcome::Skipped(format!(
            "capability not executable by the organizer: {other:?}"
        )),
    }
}

/// Move or rename a file from `from` to `to`, recording the reverse step first.
fn relocate(from: &Path, to: &Path, undo: &mut UndoJournal) -> Outcome {
    // (2) Verify state. The plan may have been approved a while ago.
    if !from.exists() {
        return Outcome::Skipped(format!("source no longer exists: {}", from.display()));
    }
    // Never overwrite. The planner de-duplicates targets, but disk state can
    // drift between plan and execution — re-check and refuse rather than clobber.
    if to.exists() {
        return Outcome::Skipped(format!(
            "target already exists, refusing to overwrite: {}",
            to.display()
        ));
    }
    // The planner emits an explicit CreateFolder for each destination, so the
    // parent normally exists by now. This is a defensive fallback only.
    if let Some(parent) = to.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Outcome::Failed(format!("could not create target parent: {e}"));
            }
        }
    }

    // (3) Undo before mutating: move the file back to where it started.
    undo.record(UndoOp::Restore {
        from: to.to_path_buf(),
        to: from.to_path_buf(),
    });

    match fs::rename(from, to) {
        // (4) Verify the result actually landed.
        Ok(()) if to.exists() && !from.exists() => Outcome::Applied,
        Ok(()) => Outcome::Failed(format!(
            "post-move verification failed for {}",
            to.display()
        )),
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::planner::plan_with_rules;
    use crate::organizer::testutil::{tempdir, TempDir};
    use std::path::{Path, PathBuf};

    fn full_rule(path: &Path) -> FolderRule {
        FolderRule {
            path: path.to_path_buf(),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
        }
    }

    /// A temp dir with a few loose files and a single full-permission rule —
    /// the in-place "organize this folder" Zone.
    fn fixture() -> (TempDir, Vec<FolderRule>) {
        let tmp = tempdir();
        tmp.file("report.pdf", b"a");
        tmp.file("photo.JPG", b"b");
        tmp.file("song.mp3", b"c");
        let rules = vec![full_rule(tmp.path())];
        (tmp, rules)
    }

    /// Sorted listing of a directory tree (relative paths), for state checks.
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
    fn applies_an_approved_plan_and_moves_files_into_category_folders() {
        let (tmp, rules) = fixture();
        let plan = plan_with_rules("z", &rules);
        let report = execute_plan(&plan, &rules);

        assert_eq!(report.failed, 0, "no action should fail in the happy path");
        assert!(
            report.applied >= 3,
            "three files + their folders should apply"
        );

        // Files now live under their category folders, gone from the root.
        let after = listing(tmp.path());
        assert!(after.contains(&"Documents/report.pdf".to_string()));
        assert!(after.contains(&"Images/photo.JPG".to_string()));
        assert!(after.contains(&"Audio/song.mp3".to_string()));
        assert!(!after.contains(&"report.pdf".to_string()));

        // Every applied action is in the audit log, and undo was recorded.
        assert_eq!(report.audit.len(), plan.actions.len());
        assert!(!report.undo.is_empty());
    }

    #[test]
    fn denied_actions_are_skipped_not_applied() {
        // Read-only source + a separate create+move destination. The planner
        // proposes moving the file out of the read-only source; policy denies it.
        let src = tempdir();
        src.file("a.pdf", b"x");
        let dest = tempdir();
        let rules = vec![FolderRule::read_only(src.path()), full_rule(dest.path())];

        let plan = plan_with_rules("z", &rules);
        let report = execute_plan(&plan, &rules);

        // The out-of-boundary move never touched disk: the file stays put.
        assert!(src.path().join("a.pdf").exists());
        assert!(report.skipped >= 1);
        assert!(report.audit.events().iter().any(|e| matches!(
            &e.outcome,
            ActionOutcome::Skipped { reason } if reason.starts_with("policy denied")
        )));
    }

    #[test]
    fn never_overwrites_an_existing_target() {
        let tmp = tempdir();
        tmp.file("report.pdf", b"new");
        // Pre-seed the destination the planner will choose, with different bytes.
        tmp.file("Documents/report.pdf", b"original");

        // The planner de-duplicates against on-disk names, so to force a genuine
        // execution-time collision we run the move capability directly.
        let mut undo = UndoJournal::new();
        let outcome = relocate(
            &tmp.path().join("report.pdf"),
            &tmp.path().join("Documents/report.pdf"),
            &mut undo,
        );
        assert!(matches!(outcome, Outcome::Skipped(_)));
        // The existing file is untouched and the source still there.
        assert_eq!(
            std::fs::read(tmp.path().join("Documents/report.pdf")).unwrap(),
            b"original"
        );
        assert!(tmp.path().join("report.pdf").exists());
        assert!(undo.is_empty(), "a refused move records no undo step");
    }

    #[test]
    fn missing_source_is_skipped_with_no_undo() {
        let tmp = tempdir();
        let mut undo = UndoJournal::new();
        let outcome = relocate(
            &tmp.path().join("ghost.pdf"), // never existed
            &tmp.path().join("Documents/ghost.pdf"),
            &mut undo,
        );
        assert!(matches!(outcome, Outcome::Skipped(_)));
        assert!(undo.is_empty());
    }

    #[test]
    fn unsupported_capabilities_are_skipped_never_executed() {
        let mut undo = UndoJournal::new();
        let outcome = apply_one(
            &Capability::DeleteFile {
                path: PathBuf::from("/anything"),
            },
            &mut undo,
        );
        assert!(matches!(outcome, Outcome::Skipped(_)));
        assert!(undo.is_empty());
    }
}
