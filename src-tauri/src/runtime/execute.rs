//! Canonical execution runtime — one pipeline for Organizer, Routines, MCP, and workflows.

use super::fs::{apply_filesystem_step, FsOutcome};
use super::receipt::{build_receipt, ExecutionReceipt};
use super::ui::{dispatch_ui_step, UiOutcome};
use super::verify::{verify_after_kind, StepVerification, VerificationStatus};
use crate::action_plan::types::ActionKind;
use crate::action_plan::types::ActionPlan;
use crate::audit::{ActionOutcome, Provenance, UndoJournal};
use crate::engine::GhostEngine;
use crate::organizer::executor::ExecutionReport;
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
pub fn execute_action_plan_with_progress(
    plan: &ActionPlan,
    rules: &[FolderRule],
    engine: Option<&GhostEngine>,
    execution_id: Option<String>,
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

fn dispatch_step(
    kind: &ActionKind,
    capability: &Capability,
    source_identity: Option<&crate::organizer::file_identity::FileIdentity>,
    rules: &[FolderRule],
    engine: Option<&GhostEngine>,
    undo: &mut UndoJournal,
) -> DispatchOutcome {
    match kind {
        ActionKind::CreateFolder { .. }
        | ActionKind::MoveFile { .. }
        | ActionKind::RenameFile { .. } => {
            match apply_filesystem_step(kind, capability, source_identity, rules, undo) {
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
        _ => match dispatch_ui_step(kind, engine) {
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
