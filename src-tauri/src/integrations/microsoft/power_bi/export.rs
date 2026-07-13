//! Pure, network-free builder for the Power BI audit-export payload.
//!
//! Turns local Organizer execution history into the row shapes documented in
//! `docs/power-bi-integration.md` (GhostRuns/GhostActions/GhostPolicyEvents).
//! No IO, no network — `commands/integrations.rs`'s preview command calls
//! this directly, and the push command re-derives the same payload
//! server-side rather than trusting anything the frontend supplied.
//!
//! Every string field is masked through `audit::pii::mask` before it leaves
//! this module, matching the redaction `organizer_export_audit` already
//! applies to Organizer audit exports (not `intelligence::redaction`, which
//! is a separate, LLM-bound pipeline for Layer C planning requests).

use crate::audit::pii::mask;
use crate::audit::{ActionOutcome, AuditEvent, Provenance};
use crate::policy::Capability;
use crate::storage::executions::StoredExecution;
use serde_json::{json, Value};

/// The three Power BI table row-sets this export produces, keyed by the
/// table names in `super::schema`.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct AuditExportPayload {
    pub runs: Vec<Value>,
    pub actions: Vec<Value>,
    pub policy_events: Vec<Value>,
}

impl AuditExportPayload {
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty() && self.actions.is_empty() && self.policy_events.is_empty()
    }
}

/// Build the export payload for a set of executions (already filtered to
/// whatever time window the caller wants — see
/// `storage::executions::list_full_executions_since`).
pub fn build_export(executions: &[StoredExecution]) -> AuditExportPayload {
    let mut payload = AuditExportPayload::default();
    for execution in executions {
        payload.runs.push(run_row(execution));
        for (index, event) in execution.audit.events().iter().enumerate() {
            payload.actions.push(action_row(execution, event, index));
            if let Some(row) = policy_event_row(execution, event, index) {
                payload.policy_events.push(row);
            }
        }
    }
    payload
}

fn run_row(execution: &StoredExecution) -> Value {
    let events = execution.audit.events();
    // Only one timestamp is persisted per execution today (`created_at`);
    // derive a best-effort `completed_at` from the latest audit event
    // instead of duplicating `created_at`, which would be even less
    // accurate for a long-running run.
    let completed_at = events
        .iter()
        .map(|e| e.at.as_str())
        .max()
        .unwrap_or(execution.created_at.as_str());

    let status = if !execution.finished {
        "interrupted"
    } else if execution.failed > 0 {
        "failed"
    } else {
        "completed"
    };

    let approved = events
        .iter()
        .filter(|e| matches!(e.outcome, ActionOutcome::Applied))
        .count();
    let denied = events
        .iter()
        .filter(|e| matches!(e.outcome, ActionOutcome::Skipped { .. }))
        .count();
    let failed = events
        .iter()
        .filter(|e| matches!(e.outcome, ActionOutcome::Failed { .. }))
        .count();

    json!({
        "run_id": execution.id,
        // Organizer has no separate "workflow" concept distinct from a Zone
        // (the schema predates the Organizer/Routines split) — the Zone id
        // is the closest available identifier, reused for both columns.
        "workflow_id": execution.zone_id,
        "workflow_name": mask(&execution.zone_id),
        "started_at": execution.created_at,
        "completed_at": completed_at,
        "status": status,
        "approved_action_count": approved,
        "denied_action_count": denied,
        "failed_action_count": failed,
        // No per-run time-saved metric exists today — send 0 rather than a
        // fabricated estimate. Known gap; see docs/power-bi-integration.md.
        "estimated_time_saved_seconds": 0,
        "undo_available": !execution.undo.is_empty(),
        // Whether a run's undo was ever invoked isn't tracked distinctly
        // from other finish paths (organizer_undo and dismissing an
        // unfinished run both just mark `finished`) — known gap, never
        // guessed at.
        "undo_used": false,
    })
}

fn action_row(execution: &StoredExecution, event: &AuditEvent, index: usize) -> Value {
    let (execution_result, detail) = match &event.outcome {
        ActionOutcome::Applied => ("applied", None),
        ActionOutcome::Skipped { reason } => ("skipped", Some(reason.as_str())),
        ActionOutcome::Failed { error } => ("failed", Some(error.as_str())),
    };
    let approval_result = match event.provenance {
        Some(Provenance::UserApproved) => "approved",
        Some(Provenance::Automated) => "automated",
        None => "n/a",
    };
    let policy_result = match &event.outcome {
        ActionOutcome::Skipped { .. } => "denied",
        _ => "allowed",
    };

    json!({
        "action_id": format!("{}-{}", execution.id, index),
        "run_id": execution.id,
        "action_type": capability_type_name(&event.capability),
        "zone_id": execution.zone_id,
        "risk_level": capability_risk_level(&event.capability),
        "policy_result": policy_result,
        "approval_result": approval_result,
        "execution_result": execution_result,
        // Not tracked per-action today — known gap, never guessed at.
        "duration_ms": 0,
        // Not one of the documented columns, but carries the masked detail
        // (skip reason / failure error) that would otherwise be silently
        // dropped; downstream consumers can ignore it.
        "detail": detail.map(mask),
    })
}

/// One row per policy-relevant outcome (skipped or failed) — a plain
/// `Applied` event has nothing to say about policy.
fn policy_event_row(
    execution: &StoredExecution,
    event: &AuditEvent,
    index: usize,
) -> Option<Value> {
    let (result, reason) = match &event.outcome {
        ActionOutcome::Skipped { reason } => ("denied", reason.as_str()),
        ActionOutcome::Failed { error } => ("failed", error.as_str()),
        ActionOutcome::Applied => return None,
    };
    let policy_rule = event
        .rule_path
        .as_ref()
        .map(|p| mask(&p.display().to_string()))
        .unwrap_or_else(|| "none".to_string());

    Some(json!({
        "event_id": format!("{}-policy-{}", execution.id, index),
        "run_id": execution.id,
        "policy_rule": policy_rule,
        "result": result,
        "reason": mask(reason),
        "created_at": event.at,
    }))
}

fn capability_type_name(capability: &Capability) -> &'static str {
    match capability {
        Capability::ReadFolder { .. } => "read_folder",
        Capability::CreateFolder { .. } => "create_folder",
        Capability::RenameFile { .. } => "rename_file",
        Capability::MoveFile { .. } => "move_file",
        Capability::CopyFile { .. } => "copy_file",
        Capability::DeleteFile { .. } => "delete_file",
        Capability::StartRecording => "start_recording",
        Capability::ReplayWorkflow { .. } => "replay_workflow",
        Capability::OsClick { .. } => "os_click",
        Capability::OsType { .. } => "os_type",
        Capability::OsShortcut { .. } => "os_shortcut",
        Capability::OsScroll => "os_scroll",
        Capability::OsWait => "os_wait",
        Capability::OsUnknown { .. } => "os_unknown",
        Capability::CaptureScreen => "capture_screen",
        Capability::UseNetwork { .. } => "use_network",
        Capability::GenerateWorkflowFromPrompt => "generate_workflow_from_prompt",
    }
}

/// Best-effort risk classification for the export only — not the same as
/// `policy::risk`, which classifies decisions, not capability kinds; no
/// existing function ties the two together at this granularity.
fn capability_risk_level(capability: &Capability) -> &'static str {
    match capability {
        Capability::ReadFolder { .. } | Capability::CreateFolder { .. } | Capability::OsWait => {
            "low"
        }
        Capability::RenameFile { .. }
        | Capability::MoveFile { .. }
        | Capability::CopyFile { .. }
        | Capability::OsScroll => "medium",
        Capability::DeleteFile { .. }
        | Capability::UseNetwork { .. }
        | Capability::OsUnknown { .. } => "high",
        Capability::StartRecording
        | Capability::ReplayWorkflow { .. }
        | Capability::OsClick { .. }
        | Capability::OsType { .. }
        | Capability::OsShortcut { .. }
        | Capability::CaptureScreen
        | Capability::GenerateWorkflowFromPrompt => "medium",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditLog, UndoJournal};
    use std::path::PathBuf;

    struct ExecutionBuilder {
        audit: AuditLog,
    }

    impl ExecutionBuilder {
        fn new() -> Self {
            ExecutionBuilder {
                audit: AuditLog::new(),
            }
        }

        fn record(mut self, capability: Capability, outcome: ActionOutcome) -> Self {
            self.audit.record_attributed(
                capability,
                outcome,
                Some(PathBuf::from("/Users/alice/Documents/Zone")),
                Some(Provenance::UserApproved),
            );
            self
        }

        fn build(self) -> StoredExecution {
            StoredExecution {
                id: "run-1".to_string(),
                zone_id: "zone-1".to_string(),
                created_at: "1000".to_string(),
                applied: 0,
                skipped: 0,
                failed: 0,
                audit: self.audit,
                undo: UndoJournal::default(),
                hash: String::new(),
                prev_hash: String::new(),
                finished: true,
                receipt: None,
            }
        }
    }

    #[test]
    fn build_export_masks_pii_in_paths_and_reasons() {
        let execution = ExecutionBuilder::new()
            .record(
                Capability::MoveFile {
                    from: PathBuf::from("/Users/alice/invoice.pdf"),
                    to: PathBuf::from("/Users/alice/Invoices/invoice.pdf"),
                },
                ActionOutcome::Skipped {
                    reason: "contact alice@example.com before moving".to_string(),
                },
            )
            .build();

        let payload = build_export(std::slice::from_ref(&execution));
        let serialized = serde_json::to_string(&payload).unwrap();
        assert!(
            !serialized.contains("alice@example.com"),
            "export payload must not leak an email address: {serialized}"
        );
    }

    #[test]
    fn build_export_counts_outcomes_correctly() {
        let execution = ExecutionBuilder::new()
            .record(
                Capability::MoveFile {
                    from: PathBuf::from("a"),
                    to: PathBuf::from("b"),
                },
                ActionOutcome::Applied,
            )
            .record(
                Capability::DeleteFile {
                    path: PathBuf::from("c"),
                },
                ActionOutcome::Skipped {
                    reason: "denied by policy".to_string(),
                },
            )
            .record(
                Capability::CopyFile {
                    from: PathBuf::from("d"),
                    to: PathBuf::from("e"),
                },
                ActionOutcome::Failed {
                    error: "disk full".to_string(),
                },
            )
            .build();

        let payload = build_export(std::slice::from_ref(&execution));
        assert_eq!(payload.runs.len(), 1);
        let run = &payload.runs[0];
        assert_eq!(run["approved_action_count"], 1);
        assert_eq!(run["denied_action_count"], 1);
        assert_eq!(run["failed_action_count"], 1);
        assert_eq!(payload.actions.len(), 3);
        // One policy row per skipped/failed event, none for the applied one.
        assert_eq!(payload.policy_events.len(), 2);
    }

    #[test]
    fn build_export_omits_raw_file_contents() {
        let execution = ExecutionBuilder::new()
            .record(
                Capability::ReadFolder {
                    path: PathBuf::from("/Users/alice/Documents"),
                },
                ActionOutcome::Applied,
            )
            .build();

        let payload = build_export(std::slice::from_ref(&execution));
        let action = &payload.actions[0];
        // Only the documented columns (plus the masked `detail` extra) should
        // be present — no raw path, no file contents.
        let obj = action.as_object().unwrap();
        for key in [
            "action_id",
            "run_id",
            "action_type",
            "zone_id",
            "risk_level",
            "policy_result",
            "approval_result",
            "execution_result",
            "duration_ms",
            "detail",
        ] {
            assert!(obj.contains_key(key), "missing expected key: {key}");
        }
        assert_eq!(obj.len(), 10);
    }

    #[test]
    fn build_export_is_empty_for_no_executions() {
        let payload = build_export(&[]);
        assert!(payload.is_empty());
    }
}
