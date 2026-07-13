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

use crate::audit::{ActionOutcome, AuditLog, Provenance, UndoJournal, UndoOp};
use crate::policy::{self, Capability, FolderRule, PolicyDecision};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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
    let mut report = ExecutionReport::default();

    for action in &plan.actions {
        let cap = &action.capability;

        // (1) Re-check policy at execution time, recording which rule fired so
        // the audit log can name the boundary that authorized (or refused) each
        // action. Deny => skip, audited, no IO.
        let evaluation = policy::evaluate_with_attribution(cap, rules);
        let rule_path = evaluation.rule_path.clone();
        if let PolicyDecision::Deny { reason } = &evaluation.decision {
            report.record_skip(cap.clone(), format!("policy denied: {reason}"), rule_path);
            on_progress(&report);
            continue;
        }
        // An `automate` rule yields Allow with no prompting; anything requiring
        // confirmation ran only because the user approved this execution.
        let provenance = if evaluation.decision.is_allowed() {
            Provenance::Automated
        } else {
            Provenance::UserApproved
        };

        match apply_one(action, rules, &mut report.undo) {
            Outcome::Applied => {
                report.applied += 1;
                report.audit.record_attributed(
                    cap.clone(),
                    ActionOutcome::Applied,
                    rule_path,
                    Some(provenance),
                );
            }
            Outcome::Skipped(reason) => report.record_skip(cap.clone(), reason, rule_path),
            Outcome::Failed(error) => {
                report.failed += 1;
                report.audit.record_attributed(
                    cap.clone(),
                    ActionOutcome::Failed { error },
                    rule_path,
                    Some(provenance),
                );
            }
        }
        on_progress(&report);
    }

    report
}

impl ExecutionReport {
    fn record_skip(&mut self, cap: Capability, reason: String, rule_path: Option<PathBuf>) {
        self.skipped += 1;
        self.audit
            .record_attributed(cap, ActionOutcome::Skipped { reason }, rule_path, None);
    }
}

/// The result of attempting a single capability.
enum Outcome {
    Applied,
    Skipped(String),
    Failed(String),
}

/// Apply one plan action, committing its undo step only after the mutation is
/// successful and verified. Only the organizer's own file-organization
/// capabilities are executable here; anything else is skipped rather than risked.
fn apply_one(
    action: &super::planner::PlanAction,
    rules: &[FolderRule],
    undo: &mut UndoJournal,
) -> Outcome {
    let cap = &action.capability;
    match cap {
        Capability::CreateFolder { path } => {
            if path.exists() {
                // Idempotent: the folder already exists (perhaps created earlier
                // in this same run). Not an error, but nothing to undo either.
                return Outcome::Skipped(format!("folder already exists: {}", path.display()));
            }
            // (3) Prepare undo before mutating: remove the folder we are about to create.
            let inverse = UndoOp::RemoveFolder { path: path.clone() };
            match fs::create_dir_all(path) {
                Ok(()) if path.is_dir() => {
                    // (4) Commit undo only after the postcondition is verified.
                    undo.record(inverse);
                    Outcome::Applied
                }
                Ok(()) => Outcome::Failed(format!("folder not created: {}", path.display())),
                Err(e) => Outcome::Failed(e.to_string()),
            }
        }
        Capability::MoveFile { from, to } => {
            relocate(from, to, action.source_identity.as_ref(), rules, true, undo)
        }
        Capability::RenameFile { from, to } => relocate(
            from,
            to,
            action.source_identity.as_ref(),
            rules,
            false,
            undo,
        ),
        other => Outcome::Skipped(format!(
            "capability not executable by the organizer: {other:?}"
        )),
    }
}

/// Move or rename a file from `from` to `to`, committing the reverse step only
/// after the move succeeds and its postcondition is verified.
fn relocate(
    from: &Path,
    to: &Path,
    expected_identity: Option<&super::file_identity::FileIdentity>,
    rules: &[FolderRule],
    is_move: bool,
    undo: &mut UndoJournal,
) -> Outcome {
    // (2) Verify state. The plan may have been approved a while ago.
    if !from.exists() {
        return Outcome::Skipped(format!("source no longer exists: {}", from.display()));
    }
    if from.is_symlink() {
        return Outcome::Skipped(format!(
            "source is a symlink, refusing to move: {}",
            from.display()
        ));
    }
    if let Some(current) = super::file_identity::FileIdentity::from_path(from) {
        if let Some(expected) = expected_identity {
            if !expected.matches(&current) {
                return Outcome::Skipped(format!(
                    "source file identity changed since plan (possible TOCTOU swap): {}",
                    from.display()
                ));
            }
        }
    } else if expected_identity.is_some() {
        return Outcome::Skipped(format!(
            "source metadata unreadable or not a regular file: {}",
            from.display()
        ));
    }
    if let Err(reason) = policy::verify_relocate_at_execution(from, to, rules, is_move) {
        return Outcome::Skipped(format!("canonical path check failed: {reason}"));
    }
    // Never overwrite. The planner de-duplicates targets, but disk state can
    // drift between plan and execution — re-check and refuse rather than clobber.
    if to.exists() {
        return Outcome::Skipped(format!(
            "target already exists, refusing to overwrite: {}",
            to.display()
        ));
    }
    // The planner emits an explicit, policy-checked CreateFolder for each
    // destination. Do not create parent folders implicitly here: doing so would
    // mutate the filesystem without a separate plan row, audit event, and undo
    // entry for that folder. If the destination parent disappeared or was
    // denied/skipped earlier in the run, skip this file rather than filling in
    // the gap behind the user's back.
    if let Some(parent) = to.parent() {
        if !parent.exists() {
            return Outcome::Skipped(format!(
                "target parent does not exist: {}",
                parent.display()
            ));
        }
    }

    // (3) Prepare undo before mutating: move the file back to where it started.
    let inverse = UndoOp::Restore {
        from: to.to_path_buf(),
        to: from.to_path_buf(),
    };

    match fs::rename(from, to) {
        // (4) Verify the result actually landed, then commit the undo entry.
        Ok(()) if to.exists() && !from.exists() => {
            undo.record(inverse);
            Outcome::Applied
        }
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
    use crate::organizer::planner::{plan_with_rules, PlanAction};
    use crate::organizer::testutil::{tempdir, TempDir};
    use std::path::{Path, PathBuf};

    fn test_action(cap: Capability) -> PlanAction {
        PlanAction {
            capability: cap,
            decision: PolicyDecision::Allow,
            rule_path: None,
            confidence: 1.0,
            reason: "test".into(),
            conflict: None,
            source_identity: None,
        }
    }

    fn relocate_in_zone(
        tmp: &TempDir,
        from: &Path,
        to: &Path,
        identity: Option<&crate::organizer::file_identity::FileIdentity>,
        is_move: bool,
        undo: &mut UndoJournal,
    ) -> Outcome {
        let rules = vec![full_rule(tmp.path())];
        relocate(from, to, identity, &rules, is_move, undo)
    }

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
    fn never_overwrites_an_existing_target() {
        let tmp = tempdir();
        tmp.file("report.pdf", b"new");
        // Pre-seed the destination the planner will choose, with different bytes.
        tmp.file("Documents/report.pdf", b"original");

        // The planner de-duplicates against on-disk names, so to force a genuine
        // execution-time collision we run the move capability directly.
        let mut undo = UndoJournal::new();
        let outcome = relocate_in_zone(
            &tmp,
            &tmp.path().join("report.pdf"),
            &tmp.path().join("Documents/report.pdf"),
            None,
            true,
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

    /// A stand-in for a real Windows file-lock error: making the target
    /// directory unwritable makes `fs::rename` fail for the same reason a
    /// locked/in-use file would on that platform, exercising the same
    /// `Outcome::Failed` path.
    #[cfg(unix)]
    #[test]
    fn apply_one_fails_when_target_dir_is_not_writable() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempdir();
        let locked_dir = tmp.dir("Documents");
        tmp.file("report.pdf", b"x");

        std::fs::set_permissions(&locked_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        // Root ignores POSIX permission bits (this dev container runs as
        // root); probe first and skip rather than assert on a false
        // premise. CI's ubuntu/macos `Test` jobs run as a normal user, so
        // the real assertion below still executes there.
        let probe = locked_dir.join("probe");
        let probe_write_succeeded = std::fs::write(&probe, b"x").is_ok();
        let _ = std::fs::remove_file(&probe);
        if probe_write_succeeded {
            std::fs::set_permissions(&locked_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            eprintln!(
                "skipping apply_one_fails_when_target_dir_is_not_writable: \
                 running as a user that bypasses permission bits (root)"
            );
            return;
        }

        let mut undo = UndoJournal::new();
        let outcome = relocate_in_zone(
            &tmp,
            &tmp.path().join("report.pdf"),
            &locked_dir.join("report.pdf"),
            None,
            true,
            &mut undo,
        );

        // Restore before any assertion can panic and skip cleanup, so
        // TempDir::drop's remove_dir_all doesn't fail afterward.
        std::fs::set_permissions(&locked_dir, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(matches!(outcome, Outcome::Failed(_)));
        assert!(undo.is_empty(), "a failed move must not create undo");
        assert!(
            tmp.path().join("report.pdf").exists(),
            "source must be left in place on failure"
        );
    }

    #[test]
    fn missing_source_is_skipped_with_no_undo() {
        let tmp = tempdir();
        let mut undo = UndoJournal::new();
        let outcome = relocate_in_zone(
            &tmp,
            &tmp.path().join("ghost.pdf"),
            &tmp.path().join("Documents/ghost.pdf"),
            None,
            true,
            &mut undo,
        );
        assert!(matches!(outcome, Outcome::Skipped(_)));
        assert!(undo.is_empty());
    }

    #[test]
    fn missing_target_parent_is_skipped_without_implicit_mutation_or_undo() {
        let tmp = tempdir();
        tmp.file("report.pdf", b"x");
        let missing_parent = tmp.path().join("Documents");
        let target = missing_parent.join("report.pdf");
        let mut undo = UndoJournal::new();

        let outcome = relocate_in_zone(
            &tmp,
            &tmp.path().join("report.pdf"),
            &target,
            None,
            true,
            &mut undo,
        );

        assert!(
            matches!(outcome, Outcome::Skipped(reason) if reason.contains("target parent does not exist"))
        );
        assert!(
            tmp.path().join("report.pdf").exists(),
            "source file must be left in place"
        );
        assert!(
            !missing_parent.exists(),
            "relocate must not create an unplanned parent folder"
        );
        assert!(undo.is_empty(), "a refused move records no undo step");
    }

    #[test]
    fn failed_folder_creation_does_not_commit_undo() {
        let tmp = tempdir();
        tmp.file("not-a-dir", b"x");
        let path = tmp.path().join("not-a-dir").join("child");
        let mut undo = UndoJournal::new();

        let outcome = apply_one(
            &test_action(Capability::CreateFolder { path }),
            &[],
            &mut undo,
        );

        assert!(matches!(outcome, Outcome::Failed(_)));
        assert!(
            undo.is_empty(),
            "failed create_dir_all must not create undo"
        );
    }

    #[test]
    fn successful_folder_creation_commits_one_matching_inverse() {
        let tmp = tempdir();
        let path = tmp.path().join("Documents");
        let mut undo = UndoJournal::new();

        let outcome = apply_one(
            &test_action(Capability::CreateFolder { path: path.clone() }),
            &[],
            &mut undo,
        );

        assert!(matches!(outcome, Outcome::Applied));
        assert_eq!(undo.ops(), &[UndoOp::RemoveFolder { path }]);
    }

    #[test]
    fn failed_move_or_rename_does_not_commit_undo() {
        let tmp = tempdir();
        tmp.file("report.pdf", b"x");
        tmp.file("not-a-dir", b"x");
        let target = tmp.path().join("not-a-dir").join("report.pdf");
        let mut undo = UndoJournal::new();

        let outcome = relocate_in_zone(
            &tmp,
            &tmp.path().join("report.pdf"),
            &target,
            None,
            true,
            &mut undo,
        );

        assert!(matches!(outcome, Outcome::Failed(_)));
        assert!(tmp.path().join("report.pdf").exists());
        assert!(undo.is_empty(), "failed rename must not create undo");
    }

    #[test]
    fn successful_move_commits_one_matching_inverse() {
        let tmp = tempdir();
        tmp.file("report.pdf", b"x");
        tmp.dir("Documents");
        let from = tmp.path().join("report.pdf");
        let to = tmp.path().join("Documents/report.pdf");
        let mut undo = UndoJournal::new();

        let outcome = relocate_in_zone(&tmp, &from, &to, None, true, &mut undo);

        assert!(matches!(outcome, Outcome::Applied));
        assert_eq!(undo.ops(), &[UndoOp::Restore { from: to, to: from }]);
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

    #[test]
    fn mixed_success_and_failure_journals_only_successful_mutations() {
        let tmp = tempdir();
        tmp.file("ok.pdf", b"ok");
        tmp.file("blocked-parent", b"x");
        tmp.dir("Documents");
        let mut undo = UndoJournal::new();

        let ok_from = tmp.path().join("ok.pdf");
        let ok_to = tmp.path().join("Documents/ok.pdf");
        let rules = vec![full_rule(tmp.path())];
        let ok = relocate(&ok_from, &ok_to, None, &rules, true, &mut undo);
        let failed = apply_one(
            &test_action(Capability::CreateFolder {
                path: tmp.path().join("blocked-parent/child"),
            }),
            &rules,
            &mut undo,
        );

        assert!(matches!(ok, Outcome::Applied));
        assert!(matches!(failed, Outcome::Failed(_)));
        assert_eq!(
            undo.ops(),
            &[UndoOp::Restore {
                from: ok_to,
                to: ok_from
            }]
        );
    }

    #[test]
    fn relocate_skips_identity_swap() {
        let tmp = tempdir();
        let path = tmp.file("report.pdf", b"original");
        let expected =
            crate::organizer::file_identity::FileIdentity::from_path(&path).expect("identity");
        tmp.dir("Documents");
        std::fs::remove_file(&path).unwrap();
        tmp.file("report.pdf", b"swapped");
        let to = tmp.path().join("Documents/report.pdf");
        let mut undo = UndoJournal::new();

        let rules = vec![full_rule(tmp.path())];
        let outcome = relocate(&path, &to, Some(&expected), &rules, true, &mut undo);

        assert!(matches!(outcome, Outcome::Skipped(reason) if reason.contains("identity changed")));
        assert!(path.exists());
        assert!(!to.exists());
        assert!(undo.is_empty());
    }

    /// Refuse to relocate symlink sources — prevents TOCTOU escape if a real
    /// file is swapped for a symlink between scan and execute.
    #[cfg(unix)]
    #[test]
    fn relocate_skips_symlink_source() {
        let tmp = tempdir();
        let real = tmp.file("secret.pdf", b"secret");
        let link = tmp.symlink_file("link.pdf", &real);
        tmp.dir("dest");
        let to = tmp.path().join("dest/link.pdf");
        let mut undo = UndoJournal::new();

        let outcome = relocate_in_zone(&tmp, &link, &to, None, true, &mut undo);

        assert!(matches!(outcome, Outcome::Skipped(_)));
        assert!(link.exists());
        assert!(!to.exists());
        assert!(undo.is_empty());
    }

    #[test]
    fn unsupported_capabilities_are_skipped_never_executed() {
        let mut undo = UndoJournal::new();
        let outcome = apply_one(
            &test_action(Capability::DeleteFile {
                path: PathBuf::from("/anything"),
            }),
            &[],
            &mut undo,
        );
        assert!(matches!(outcome, Outcome::Skipped(_)));
        assert!(undo.is_empty());
    }
}
