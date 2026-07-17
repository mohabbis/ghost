//! Canonical execution runtime — one pipeline for Organizer, Routines, MCP, and workflows.

use super::fs::{FsOutcome, apply_filesystem_step};
use super::receipt::{ExecutionReceipt, build_receipt};
use super::ui::{UiOutcome, dispatch_ui_step_with_reliability};
use super::verify::{StepVerification, VerificationStatus, verify_after_kind};
use crate::action_plan::types::ActionKind;
use crate::action_plan::types::ActionPlan;
use crate::audit::{ActionOutcome, Provenance, UndoJournal};
use crate::core::events::ReliabilitySettings;
use crate::engine::GhostEngine;
use crate::organizer::executor::ExecutionReport;
use crate::organizer::trash::{OsTrasher, Trasher};
use crate::policy::{self, Capability, FolderRule, PolicyDecision};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct RuntimeResult {
    pub report: ExecutionReport,
    pub verifications: Vec<StepVerification>,
    pub receipt: ExecutionReceipt,
    pub stopped_early: bool,
    pub stop_reason: Option<String>,
}

fn now_epoch_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".into())
}

/// Execute an approved [`ActionPlan`] through policy → dispatch → verify → audit.
/// Deletes (if any) route through the real OS trash ([`OsTrasher`]).
pub fn execute_action_plan_with_progress(
    plan: &ActionPlan,
    rules: &[FolderRule],
    engine: Option<&GhostEngine>,
    execution_id: Option<String>,
    on_progress: impl FnMut(&ExecutionReport),
) -> RuntimeResult {
    execute_action_plan_with_options(
        plan,
        rules,
        engine,
        execution_id,
        None,
        &OsTrasher,
        on_progress,
    )
}

/// Like [`execute_action_plan_with_progress`], but enables reliability retries for UI replay steps.
pub fn execute_action_plan_with_reliability(
    plan: &ActionPlan,
    rules: &[FolderRule],
    engine: Option<&GhostEngine>,
    execution_id: Option<String>,
    reliability: &ReliabilitySettings,
    on_progress: impl FnMut(&ExecutionReport),
) -> RuntimeResult {
    execute_action_plan_with_options(
        plan,
        rules,
        engine,
        execution_id,
        Some(reliability),
        &OsTrasher,
        on_progress,
    )
}

fn execute_action_plan_with_options(
    plan: &ActionPlan,
    rules: &[FolderRule],
    engine: Option<&GhostEngine>,
    execution_id: Option<String>,
    reliability: Option<&ReliabilitySettings>,
    trasher: &dyn Trasher,
    mut on_progress: impl FnMut(&ExecutionReport),
) -> RuntimeResult {
    let started_at = now_epoch_string();
    let mut report = ExecutionReport::default();
    let mut verifications = Vec::with_capacity(plan.steps.len());
    let mut stopped_early = false;
    let mut stop_reason = None;

    for step in &plan.steps {
        let evaluation = policy::evaluate_with_attribution(&step.capability, rules);
        let rule_path = evaluation.rule_path.clone();

        if let PolicyDecision::Deny { reason } = &evaluation.decision {
            record_skip(
                &mut report,
                step.capability.clone(),
                format!("policy denied: {reason}"),
                rule_path,
            );
            verifications.push(StepVerification::skipped(
                &step.id,
                &step.label,
                &format!("policy denied: {reason}"),
            ));
            on_progress(&report);
            continue;
        }

        let provenance = if evaluation.decision.is_allowed() {
            Provenance::Automated
        } else {
            Provenance::UserApproved
        };

        let outcome = dispatch_step(
            &step.kind,
            &step.capability,
            step.source_identity.as_ref(),
            rules,
            engine,
            reliability,
            trasher,
            &mut report.undo,
        );

        match &outcome {
            DispatchOutcome::Applied => {
                report.applied += 1;
                report.audit.record_attributed(
                    step.capability.clone(),
                    ActionOutcome::Applied,
                    rule_path,
                    Some(provenance),
                );
            }
            DispatchOutcome::Skipped(reason) => {
                record_skip(
                    &mut report,
                    step.capability.clone(),
                    reason.clone(),
                    rule_path,
                );
            }
            DispatchOutcome::Failed(error) => {
                report.failed += 1;
                report.audit.record_attributed(
                    step.capability.clone(),
                    ActionOutcome::Failed {
                        error: error.clone(),
                    },
                    rule_path,
                    Some(provenance),
                );
            }
        }

        let mut verification = verify_after_kind(&step.kind, &step.id, &step.label);
        if matches!(outcome, DispatchOutcome::Failed(_)) {
            verification.continue_execution = false;
            verification.status = VerificationStatus::Failed;
            stopped_early = true;
            stop_reason = Some(format!("step {} failed", step.label));
        }
        verifications.push(verification.clone());
        on_progress(&report);

        if !verification.continue_execution {
            break;
        }
    }

    let finished_at = now_epoch_string();
    let receipt = build_receipt(
        &plan.id,
        &plan.title,
        execution_id,
        &report,
        &verifications,
        &started_at,
        &finished_at,
        stopped_early,
        stop_reason.clone(),
    );

    RuntimeResult {
        report,
        verifications,
        receipt,
        stopped_early,
        stop_reason,
    }
}

enum DispatchOutcome {
    Applied,
    Skipped(String),
    Failed(String),
}

#[allow(clippy::too_many_arguments)]
fn dispatch_step(
    kind: &ActionKind,
    capability: &Capability,
    source_identity: Option<&crate::organizer::file_identity::FileIdentity>,
    rules: &[FolderRule],
    engine: Option<&GhostEngine>,
    reliability: Option<&ReliabilitySettings>,
    trasher: &dyn Trasher,
    undo: &mut UndoJournal,
) -> DispatchOutcome {
    match kind {
        ActionKind::CreateFolder { .. }
        | ActionKind::MoveFile { .. }
        | ActionKind::RenameFile { .. }
        | ActionKind::DeleteFile { .. } => {
            match apply_filesystem_step(kind, capability, source_identity, rules, trasher, undo) {
                FsOutcome::Applied => DispatchOutcome::Applied,
                FsOutcome::Skipped(r) => DispatchOutcome::Skipped(r),
                FsOutcome::Failed(e) => DispatchOutcome::Failed(e),
            }
        }
        ActionKind::VerifyPath { path, should_exist } => {
            if path.exists() == *should_exist {
                DispatchOutcome::Applied
            } else {
                DispatchOutcome::Failed(format!("verification failed for {}", path.display()))
            }
        }
        _ => match dispatch_ui_step_with_reliability(kind, engine, reliability) {
            UiOutcome::Applied => DispatchOutcome::Applied,
            UiOutcome::Skipped(r) => DispatchOutcome::Skipped(r),
            UiOutcome::Failed(e) => DispatchOutcome::Failed(e),
        },
    }
}

fn record_skip(
    report: &mut ExecutionReport,
    cap: Capability,
    reason: String,
    rule_path: Option<std::path::PathBuf>,
) {
    report.skipped += 1;
    report
        .audit
        .record_attributed(cap, ActionOutcome::Skipped { reason }, rule_path, None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_plan::types::{ActionKind, ActionStep, PlanSource};
    use crate::organizer::testutil::tempdir;
    use crate::policy::PolicyDecision;

    fn fs_step(id: &str, label: &str, kind: ActionKind, cap: Capability) -> ActionStep {
        ActionStep {
            id: id.into(),
            label: label.into(),
            kind,
            capability: cap,
            decision: PolicyDecision::Allow,
            rule_path: None,
            confidence: 1.0,
            reason: String::new(),
            source_identity: None,
        }
    }

    #[test]
    fn verification_failure_stops_execution() {
        let tmp = tempdir();
        let missing = tmp.path().join("nope.pdf");
        let rules = vec![crate::policy::FolderRule {
            path: tmp.path().to_path_buf(),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
            trust: crate::policy::TrustLevel::Automate,
        }];
        let plan = ActionPlan::new(
            "t".into(),
            "test".into(),
            PlanSource::Demo,
            vec![fs_step(
                "v1",
                "verify missing",
                ActionKind::VerifyPath {
                    path: missing.clone(),
                    should_exist: true,
                },
                Capability::ReadFolder { path: missing },
            )],
        );
        let result = execute_action_plan_with_progress(&plan, &rules, None, None, |_| {});
        assert!(result.stopped_early);
        assert_eq!(result.report.failed, 1);
    }

    fn rule(path: &std::path::Path, can_delete: bool) -> crate::policy::FolderRule {
        crate::policy::FolderRule {
            path: path.to_path_buf(),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete,
            trust: crate::policy::TrustLevel::Automate,
        }
    }

    fn delete_plan(path: &std::path::Path) -> ActionPlan {
        ActionPlan::new(
            "t".into(),
            "delete".into(),
            PlanSource::Demo,
            vec![fs_step(
                "d1",
                "delete junk",
                ActionKind::DeleteFile {
                    path: path.to_path_buf(),
                },
                Capability::DeleteFile {
                    path: path.to_path_buf(),
                },
            )],
        )
    }

    #[test]
    fn delete_without_can_delete_grant_is_policy_denied_and_skipped() {
        let tmp = tempdir();
        let f = tmp.file("junk.tmp", b"x");
        let rules = vec![rule(tmp.path(), false)];
        let trasher =
            crate::organizer::trash::test_support::RecordingTrasher::new(tmp.path().join(".hold"));
        let result = execute_action_plan_with_options(
            &delete_plan(&f),
            &rules,
            None,
            None,
            None,
            &trasher,
            |_| {},
        );
        assert_eq!(result.report.skipped, 1);
        assert_eq!(result.report.applied, 0);
        assert!(f.exists(), "denied delete must leave the file untouched");
        assert!(trasher.trashed_paths().is_empty());
        assert!(result.report.undo.is_empty());
        assert_eq!(
            result.verifications[0].status,
            VerificationStatus::Skipped,
            "a policy-denied delete is a skip, not a hard failure"
        );
    }

    #[test]
    fn delete_with_grant_routes_through_trash_verifies_absence_and_records_undo() {
        let tmp = tempdir();
        let f = tmp.file("junk.tmp", b"x");
        let rules = vec![rule(tmp.path(), true)];
        // The plan's policy decision is RequireConfirmation (High risk, never
        // silent); execute_action_plan runs only after that approval upstream.
        let trasher =
            crate::organizer::trash::test_support::RecordingTrasher::new(tmp.path().join(".hold"));
        let result = execute_action_plan_with_options(
            &delete_plan(&f),
            &rules,
            None,
            None,
            None,
            &trasher,
            |_| {},
        );
        assert_eq!(result.report.applied, 1);
        assert!(!f.exists(), "source must be gone after the delete");
        assert_eq!(trasher.trashed_paths(), vec![f.clone()]);
        assert!(matches!(
            result.report.undo.ops(),
            [crate::audit::UndoOp::Untrash { original_path, .. }] if *original_path == f
        ));
        assert_eq!(result.verifications[0].status, VerificationStatus::Verified);
        assert!(!result.stopped_early);
    }

    #[test]
    fn delete_toctou_swap_is_skipped_and_file_kept() {
        let tmp = tempdir();
        let f = tmp.file("junk.tmp", b"original");
        let identity =
            crate::organizer::file_identity::FileIdentity::from_path(&f).expect("identity");
        let rules = vec![rule(tmp.path(), true)];
        let mut plan = delete_plan(&f);
        plan.steps[0].source_identity = Some(identity);
        // Swap the file after planning: same path, different inode/contents.
        std::fs::remove_file(&f).unwrap();
        tmp.file("junk.tmp", b"swapped");
        let trasher =
            crate::organizer::trash::test_support::RecordingTrasher::new(tmp.path().join(".hold"));
        let result =
            execute_action_plan_with_options(&plan, &rules, None, None, None, &trasher, |_| {});
        assert_eq!(result.report.skipped, 1);
        assert_eq!(result.report.applied, 0);
        assert!(f.exists(), "swapped file must not be deleted");
        assert!(trasher.trashed_paths().is_empty());
        assert!(result.report.undo.is_empty());
    }

    #[test]
    fn filesystem_step_verifies_after_apply() {
        let tmp = tempdir();
        let folder = tmp.path().join("newdir");
        let rules = vec![crate::policy::FolderRule {
            path: tmp.path().to_path_buf(),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
            trust: crate::policy::TrustLevel::Automate,
        }];
        let plan = ActionPlan::new(
            "t".into(),
            "mkdir".into(),
            PlanSource::Demo,
            vec![fs_step(
                "f1",
                "create",
                ActionKind::CreateFolder {
                    path: folder.clone(),
                },
                Capability::CreateFolder {
                    path: folder.clone(),
                },
            )],
        );
        let result = execute_action_plan_with_progress(&plan, &rules, None, None, |_| {});
        assert_eq!(result.report.applied, 1);
        assert!(folder.is_dir());
        assert!(!result.receipt.steps.is_empty());
    }
}
