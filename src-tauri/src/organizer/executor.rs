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
//! 3. **Prepares undo before mutating.** A reversible op constructs its inverse
//!    before the filesystem call, but does not commit it to the [`UndoJournal`] yet.
//! 4. **Applies, verifies the result, then commits undo**, and records one audit event.
//!
//! It only ever applies the file-organization capabilities the planner emits
//! (`CreateFolder`, `MoveFile`, `RenameFile`). It never deletes, never copies
//! over an existing file, and a failure on one action leaves every other file
//! recoverable — partial progress is captured in the audit log and reversible
//! through the journal.

use crate::audit::{AuditLog, UndoJournal};
use crate::policy::FolderRule;
use serde::{Deserialize, Serialize};

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
///
/// Holds the whole run's report in memory until it returns, so it's only
/// crash-safe as a unit — a mid-run crash loses the undo journal for
/// whatever had already been applied. Callers that persist the report as it
/// grows (write-ahead durability) should use [`execute_plan_with_progress`]
/// instead; this is a thin wrapper over it with a no-op callback, kept for
/// call sites (and the tests below) that don't need that.
pub fn execute_plan(plan: &OrganizerPlan, rules: &[FolderRule]) -> ExecutionReport {
    execute_plan_with_progress(plan, rules, |_| {})
}

/// Like [`execute_plan`], but invokes `on_progress` with the report-so-far
/// after every action — applied, skipped, or failed alike, since a crash
/// recovery view benefits from an accurate picture of what was attempted,
/// not just what mutated.
///
/// `on_progress` is deliberately given the *whole* report each time (not just
/// the newest step): the natural way to persist it is a durable snapshot
/// overwrite (`storage::executions::update_execution_progress`), simpler and
/// less failure-prone than an append-only log the caller would have to
/// replay correctly.
pub fn execute_plan_with_progress(
    plan: &OrganizerPlan,
    rules: &[FolderRule],
    mut on_progress: impl FnMut(&ExecutionReport),
) -> ExecutionReport {
    let action_plan = crate::action_plan::from_organizer_plan(plan);
    crate::runtime::execute_action_plan_with_progress(&action_plan, rules, None, None, |report| {
        on_progress(report)
    })
    .report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{ActionOutcome, Provenance};
    use crate::organizer::planner::plan_with_rules;
    use crate::organizer::testutil::{TempDir, tempdir};
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
                    // Normalize to `/` so assertions are separator-agnostic
                    // (Path::display emits `\` on Windows).
                    let rel = p.strip_prefix(base).unwrap().display().to_string();
                    out.push(rel.replace('\\', "/"));
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
    fn progress_callback_fires_once_per_action_with_growing_state() {
        // `_tmp` still needs to stay alive (its Drop cleans up the temp
        // dir); the test itself only reads `rules`.
        let (_tmp, rules) = fixture();
        let plan = plan_with_rules("z", &rules);
        let action_count = plan.actions.len();

        let mut snapshots: Vec<(usize, usize, usize)> = Vec::new();
        let report = execute_plan_with_progress(&plan, &rules, |r| {
            snapshots.push((r.applied, r.skipped, r.failed));
        });

        assert_eq!(
            snapshots.len(),
            action_count,
            "callback must fire exactly once per action"
        );
        // Monotonically non-decreasing totals — each snapshot is a growing
        // prefix of the final report, never a regression.
        let mut prev = (0, 0, 0);
        for s in &snapshots {
            assert!(s.0 >= prev.0 && s.1 >= prev.1 && s.2 >= prev.2);
            prev = *s;
        }
        assert_eq!(
            *snapshots.last().unwrap(),
            (report.applied, report.skipped, report.failed),
            "the last snapshot must match the final report"
        );
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
    fn applied_actions_record_the_rule_that_fired_and_provenance() {
        // A default (ask_first) full-permission rule: applied moves ran only
        // because the user approved the run -> UserApproved, attributed to the
        // rule covering the destination.
        let (tmp, rules) = fixture();
        let plan = plan_with_rules("z", &rules);
        let report = execute_plan(&plan, &rules);

        let applied_moves: Vec<&crate::audit::AuditEvent> = report
            .audit
            .events()
            .iter()
            .filter(|e| {
                matches!(e.capability, Capability::MoveFile { .. })
                    && matches!(e.outcome, ActionOutcome::Applied)
            })
            .collect();
        assert!(!applied_moves.is_empty());
        for event in applied_moves {
            assert_eq!(event.rule_path.as_deref(), Some(tmp.path()));
            assert_eq!(event.provenance, Some(Provenance::UserApproved));
        }
    }

    #[test]
    fn automate_rule_records_automated_provenance() {
        let tmp = tempdir();
        tmp.file("report.pdf", b"a");
        let rules = vec![FolderRule {
            trust: crate::policy::TrustLevel::Automate,
            ..full_rule(tmp.path())
        }];

        let plan = plan_with_rules("z", &rules);
        let report = execute_plan(&plan, &rules);
        assert!(report.applied >= 1);
        assert!(report.audit.events().iter().any(|e| {
            matches!(e.outcome, ActionOutcome::Applied)
                && e.provenance == Some(Provenance::Automated)
        }));
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
    fn unicode_filename_round_trips_through_plan_and_execute() {
        let tmp = tempdir();
        tmp.file("日本語 📷 café.pdf", b"x");
        let rules = vec![full_rule(tmp.path())];
        let plan = plan_with_rules("z", &rules);
        let report = execute_plan(&plan, &rules);

        assert_eq!(report.failed, 0);
        let after = listing(tmp.path());
        assert!(
            after.contains(&"Documents/日本語 📷 café.pdf".to_string()),
            "Unicode filename must survive move byte-for-byte: {after:?}"
        );
    }

    #[test]
    fn applied_audit_count_matches_committed_undo_entries() {
        let (tmp, rules) = fixture();
        let plan = plan_with_rules("z", &rules);
        let report = execute_plan(&plan, &rules);
        let applied_audit_count = report
            .audit
            .events()
            .iter()
            .filter(|event| matches!(event.outcome, ActionOutcome::Applied))
            .count();

        assert_eq!(report.applied, applied_audit_count);
        assert_eq!(report.applied, report.undo.len());
        assert!(tmp.path().join("Documents/report.pdf").exists());
    }
}
