//! Canonical execution runtime — one pipeline for Organizer, Routines, MCP, and workflows.

use super::evidence::StepEvidence;
use super::fs::{FsOutcome, apply_filesystem_step};
use super::helper_budget::HelperBudget;
use super::receipt::{ExecutionReceipt, build_receipt};
use super::ui::{UiOutcome, dispatch_ui_step_with_budget};
use super::verify::{StepVerification, VerificationStatus, verify_after_kind};
use crate::action_plan::reliability::{
    backoff_duration, effective_max_attempts, effective_retry_policy, effective_timeout_ms,
};
use crate::action_plan::types::{ActionKind, ActionPlan, ActionStep, StepCondition};
use crate::audit::{ActionOutcome, Provenance, UndoJournal};
use crate::core::events::ReliabilitySettings;
use crate::engine::GhostEngine;
use crate::organizer::executor::ExecutionReport;
use crate::organizer::trash::{OsTrasher, Trasher};
use crate::policy::{self, Capability, FolderRule, PolicyDecision};
use crate::runtime::semantic::{self, UiTarget};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

        let outcome = execute_step_with_reliability(
            step,
            rules,
            engine,
            reliability,
            trasher,
            &mut report.undo,
        );

        let evidence = match &outcome {
            DispatchOutcome::Applied(ev) => {
                report.applied += 1;
                report.audit.record_attributed(
                    step.capability.clone(),
                    ActionOutcome::Applied,
                    rule_path,
                    Some(provenance),
                );
                ev.clone()
            }
            DispatchOutcome::Skipped(reason) => {
                record_skip(
                    &mut report,
                    step.capability.clone(),
                    reason.clone(),
                    rule_path,
                );
                None
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
                None
            }
        };

        let mut verification = verify_after_kind(&step.kind, &step.id, &step.label, evidence);
        if matches!(outcome, DispatchOutcome::Failed(_)) {
            verification.continue_execution = false;
            verification.status = VerificationStatus::Failed;
            stopped_early = true;
            stop_reason = Some(format!("step {} failed", step.label));
        } else if !verification.continue_execution {
            // Verify mismatch (or other hard postcondition fail): seal the halt on the receipt.
            stopped_early = true;
            stop_reason = Some(format!(
                "verification halted on {}: expected «{}» · observed «{}»",
                step.label, verification.expected, verification.observed
            ));
            if matches!(
                verification.status,
                VerificationStatus::Verified | VerificationStatus::NotApplicable
            ) {
                verification.status = VerificationStatus::Failed;
            }
            report.failed += 1;
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
    Applied(Option<StepEvidence>),
    Skipped(String),
    Failed(String),
}

/// Run declared preconditions, then dispatch with timeout + risk-aware retries.
fn execute_step_with_reliability(
    step: &ActionStep,
    rules: &[FolderRule],
    engine: Option<&GhostEngine>,
    reliability: Option<&ReliabilitySettings>,
    trasher: &dyn Trasher,
    undo: &mut UndoJournal,
) -> DispatchOutcome {
    if let Err(reason) = check_preconditions(&step.preconditions) {
        return DispatchOutcome::Failed(reason);
    }

    let retry = effective_retry_policy(&step.kind, &step.capability, step.retry_policy.as_ref());
    let max_attempts = effective_max_attempts(&retry);
    let timeout = effective_timeout_ms(&step.kind, step.timeout_ms).map(Duration::from_millis);
    let started = Instant::now();

    let mut attempt = 0u32;
    let mut last_failure = String::from("step failed");
    while attempt < max_attempts {
        if let Some(limit) = timeout
            && started.elapsed() >= limit
        {
            return DispatchOutcome::Failed(format!(
                "step timed out after {} ms ({last_failure})",
                limit.as_millis()
            ));
        }

        let budget = HelperBudget::from_step_timeout(started, timeout);
        let outcome = dispatch_step(
            &step.kind,
            &step.capability,
            step.source_identity.as_ref(),
            rules,
            engine,
            reliability,
            trasher,
            undo,
            budget,
        );
        match outcome {
            DispatchOutcome::Applied(evidence) => {
                if let Err(reason) = check_postconditions(&step.postconditions) {
                    return DispatchOutcome::Failed(reason);
                }
                let attempts_used = attempt + 1;
                let evidence = evidence
                    .map(|ev| ev.with_attempts(attempts_used))
                    .or_else(|| {
                        if attempts_used > 1 {
                            Some(StepEvidence::default().with_attempts(attempts_used))
                        } else {
                            None
                        }
                    });
                return DispatchOutcome::Applied(evidence);
            }
            DispatchOutcome::Skipped(reason) => return DispatchOutcome::Skipped(reason),
            DispatchOutcome::Failed(error) => {
                last_failure = error;
                attempt += 1;
                if attempt >= max_attempts {
                    break;
                }
                if let Some(limit) = timeout {
                    let remaining = limit.saturating_sub(started.elapsed());
                    if remaining.is_zero() {
                        break;
                    }
                    let sleep_for = backoff_duration(&retry, attempt - 1).min(remaining);
                    std::thread::sleep(sleep_for);
                } else {
                    std::thread::sleep(backoff_duration(&retry, attempt - 1));
                }
            }
        }
    }

    if let Some(limit) = timeout
        && started.elapsed() >= limit
    {
        return DispatchOutcome::Failed(format!(
            "step timed out after {} ms ({last_failure})",
            limit.as_millis()
        ));
    }
    if max_attempts > 1 {
        DispatchOutcome::Failed(format!("{last_failure} (after {max_attempts} attempts)"))
    } else {
        DispatchOutcome::Failed(last_failure)
    }
}

fn check_preconditions(conditions: &[StepCondition]) -> Result<(), String> {
    for condition in conditions {
        match condition {
            StepCondition::AppRunning { app, bundle_id } => {
                let mut target = UiTarget::new(app.clone(), "");
                target.bundle_id = bundle_id.clone();
                semantic::ensure_app_running(&target).map_err(|e| e.to_string())?;
            }
            StepCondition::PathExists { path } => {
                if !path.exists() {
                    return Err(format!(
                        "precondition failed: path missing {}",
                        path.display()
                    ));
                }
            }
            StepCondition::PathAbsent { path } => {
                if path.exists() {
                    return Err(format!(
                        "precondition failed: path already exists {}",
                        path.display()
                    ));
                }
            }
            StepCondition::SemanticTarget {
                target,
                expected_value,
            } => {
                // Soft check only — UI observation is not business-effect proof.
                let _ = semantic::verify_postcondition(target, expected_value.as_deref());
            }
        }
    }
    Ok(())
}

fn check_postconditions(conditions: &[StepCondition]) -> Result<(), String> {
    for condition in conditions {
        match condition {
            StepCondition::PathExists { path } => {
                if !path.exists() {
                    return Err(format!(
                        "postcondition failed: path missing {}",
                        path.display()
                    ));
                }
            }
            StepCondition::PathAbsent { path } => {
                if path.exists() {
                    return Err(format!(
                        "postcondition failed: path still present {}",
                        path.display()
                    ));
                }
            }
            StepCondition::AppRunning { .. } | StepCondition::SemanticTarget { .. } => {
                // UI postconditions stay best-effort; hard FS checks above are authoritative.
            }
        }
    }
    Ok(())
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
    budget: Option<HelperBudget>,
) -> DispatchOutcome {
    match kind {
        ActionKind::CreateFolder { .. }
        | ActionKind::MoveFile { .. }
        | ActionKind::RenameFile { .. }
        | ActionKind::DeleteFile { .. } => {
            match apply_filesystem_step(kind, capability, source_identity, rules, trasher, undo) {
                FsOutcome::Applied => {
                    DispatchOutcome::Applied(Some(StepEvidence::filesystem_undo_recorded()))
                }
                FsOutcome::Skipped(r) => DispatchOutcome::Skipped(r),
                FsOutcome::Failed(e) => DispatchOutcome::Failed(e),
            }
        }
        ActionKind::VerifyPath { path, should_exist } => {
            if path.exists() == *should_exist {
                DispatchOutcome::Applied(None)
            } else {
                DispatchOutcome::Failed(format!("verification failed for {}", path.display()))
            }
        }
        _ => match dispatch_ui_step_with_budget(kind, engine, reliability, budget) {
            UiOutcome::Applied(ev) => DispatchOutcome::Applied(ev),
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
        ActionStep::new(id, label, kind, cap, PolicyDecision::Allow)
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
        assert!(
            result
                .stop_reason
                .as_deref()
                .is_some_and(|r| r.contains("verify missing")),
            "expected stop_reason to name the failed step, got {:?}",
            result.stop_reason
        );
        assert!(result.receipt.stopped_early);
        assert_eq!(result.receipt.stop_reason, result.stop_reason);
    }

    #[test]
    fn postcondition_verify_halt_sets_receipt_stop_reason() {
        // CreateFolder applies, then VerifyPath (should_exist=false) on that folder
        // fails in dispatch — still seals stopped_early + stop_reason on the receipt.
        // Semantic value-mismatch halt uses the same seal path when continue_execution
        // is false after an Applied dispatch (covered by runtime::verify unit tests).
        let tmp = tempdir();
        let folder = tmp.path().join("created");
        let rules = vec![rule(tmp.path(), false)];
        let plan = ActionPlan::new(
            "t".into(),
            "test".into(),
            PlanSource::Demo,
            vec![
                fs_step(
                    "c1",
                    "create folder",
                    ActionKind::CreateFolder {
                        path: folder.clone(),
                    },
                    Capability::CreateFolder {
                        path: folder.clone(),
                    },
                ),
                fs_step(
                    "v1",
                    "verify absent",
                    ActionKind::VerifyPath {
                        path: folder.clone(),
                        should_exist: false,
                    },
                    Capability::ReadFolder { path: folder },
                ),
            ],
        );
        let result = execute_action_plan_with_progress(&plan, &rules, None, None, |_| {});
        assert_eq!(result.report.applied, 1);
        assert!(result.stopped_early);
        assert!(result.receipt.stopped_early);
        assert!(
            result
                .stop_reason
                .as_deref()
                .is_some_and(|r| r.contains("verify absent")),
            "got {:?}",
            result.stop_reason
        );
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
        assert!(
            result.receipt.steps[0]
                .undo_note
                .as_deref()
                .unwrap_or("")
                .contains("undo journal"),
            "FS apply should note undo journal on receipt"
        );
    }

    #[test]
    fn ui_wait_step_records_coordinates_strategy_and_n_a_undo() {
        use crate::action_plan::ActionKind;
        let plan = ActionPlan::new(
            "t".into(),
            "wait".into(),
            PlanSource::Demo,
            vec![ActionStep::new(
                "w1",
                "wait briefly",
                ActionKind::Wait { ms: 1 },
                Capability::OsWait,
                PolicyDecision::Allow,
            )],
        );
        let result = execute_action_plan_with_progress(&plan, &[], None, None, |_| {});
        assert_eq!(result.report.applied, 1);
        assert_eq!(
            result.receipt.steps[0].resolution_strategy,
            Some(crate::runtime::UiResolutionStrategy::Coordinates)
        );
        assert!(
            result.receipt.steps[0]
                .undo_note
                .as_deref()
                .unwrap_or("")
                .contains("n/a"),
            "UI steps must not claim journal undo"
        );
    }

    #[test]
    fn old_action_step_json_deserializes_without_reliability_fields() {
        let json = r#"{
            "id": "s1",
            "label": "wait",
            "kind": { "kind": "wait", "ms": 10 },
            "capability": { "kind": "os_wait" },
            "decision": { "decision": "allow" }
        }"#;
        let step: ActionStep = serde_json::from_str(json).expect("old step JSON");
        assert!(step.retry_policy.is_none());
        assert!(step.timeout_ms.is_none());
        assert!(step.preconditions.is_empty());
        assert_eq!(
            step.fallback_strategy,
            crate::action_plan::FallbackStrategy::None
        );
    }

    #[test]
    fn verify_failure_can_retry_then_stop() {
        use crate::action_plan::{RetryClass, RetryPolicy};
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
        let mut step = fs_step(
            "v1",
            "verify missing",
            ActionKind::VerifyPath {
                path: missing.clone(),
                should_exist: true,
            },
            Capability::ReadFolder { path: missing },
        );
        step.retry_policy = Some(RetryPolicy {
            class: RetryClass::Allowed,
            max_attempts: 2,
            backoff_ms: 20,
        });
        step.timeout_ms = Some(5_000);
        let plan = ActionPlan::new("t".into(), "test".into(), PlanSource::Demo, vec![step]);
        let result = execute_action_plan_with_progress(&plan, &rules, None, None, |_| {});
        assert!(result.stopped_early);
        assert_eq!(result.report.failed, 1);
        let err = result
            .report
            .audit
            .events()
            .iter()
            .find_map(|e| match &e.outcome {
                ActionOutcome::Failed { error } => Some(error.as_str()),
                _ => None,
            });
        assert!(
            err.is_some_and(|e| e.contains("after 2 attempts")),
            "expected retry marker, got {err:?}"
        );
    }
}
