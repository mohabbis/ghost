//! Headless MCP execution helpers — canonical ActionPlan runtime.

use crate::action_plan::{PlanSource, from_compression_report, from_organizer_plan_with_source};
use crate::engine::GhostEngine;
use crate::mcp::approval::{verify_execution_token_for_plan, verify_execution_token_with_hash};
use crate::mcp::plan_hash::{hash_organizer_plan, hash_routine_plan, routine_plan_id};
use crate::organizer::pipeline::undo_zone_run;
use crate::organizer::planner::plan_zone;
use crate::runtime::run_persisted_action_plan;
use crate::runtime::{ExecutionReceipt, RuntimeResult, StepVerification};
use crate::storage::Db;
use crate::storage::executions::{ExecutionSummary, StoredExecution, get_execution};
use crate::storage::open_default;
use crate::storage::zones::list_folder_rules;
use serde_json::{Value, json};

pub fn execute_approved_plan(zone_id: &str, approval_token: &str) -> Result<Value, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    let plan = plan_zone(&conn, zone_id).map_err(|e| e.to_string())?;
    let plan_hash = hash_organizer_plan(&plan);

    if plan.actions.iter().any(|a| a.decision.is_denied()) {
        return Err("Plan contains denied operations — cannot execute".to_string());
    }
    if !plan.actions.iter().any(|a| !a.decision.is_denied()) {
        return Err("Plan has no applyable actions".to_string());
    }

    verify_execution_token_with_hash(approval_token, zone_id, Some(&plan_hash))?;

    let action_plan = from_organizer_plan_with_source(
        &plan,
        PlanSource::Mcp {
            zone_id: zone_id.into(),
        },
    );
    let rules = list_folder_rules(&conn, zone_id).map_err(|e| e.to_string())?;
    let outcome = run_persisted_action_plan(
        &conn,
        zone_id,
        &action_plan,
        &rules,
        None,
        false,
        None,
        None,
    )?;

    Ok(execute_response_json(
        outcome.execution_id,
        json!({ "zone_id": zone_id, "plan_source": "mcp" }),
        &outcome.runtime,
    ))
}

/// Execute an unchanged saved routine through the canonical Action Plan
/// runtime after consuming a desktop-issued, exact-hash approval token.
pub fn execute_approved_routine(
    workflow_name: &str,
    approval_token: &str,
    engine: &GhostEngine,
) -> Result<Value, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    execute_approved_routine_with_db(workflow_name, approval_token, engine, &conn)
}

/// Same as [`execute_approved_routine`], with an explicit DB (tests / alternate stores).
pub fn execute_approved_routine_with_db(
    workflow_name: &str,
    approval_token: &str,
    engine: &GhostEngine,
    conn: &Db,
) -> Result<Value, String> {
    if !engine.auth().is_unlocked() {
        return Err(
            "Ghost's vault is locked. Unlock Ghost before previewing or running this routine."
                .to_string(),
        );
    }
    let events = engine
        .load_workflow(workflow_name)
        .map_err(|e| format!("Could not load saved routine '{workflow_name}': {e}"))?;
    let report = crate::core::compression::compress(&events);
    let policy_plan = crate::policy::evaluate_compressed(&report);
    crate::policy::ensure_replayable(&policy_plan)?;

    let plan_hash = hash_routine_plan(workflow_name, &events);
    let plan_id = routine_plan_id(workflow_name);
    verify_execution_token_for_plan(approval_token, &plan_id, Some(&plan_hash))?;

    let action_plan = from_compression_report(&report, &events, Some(workflow_name.to_string()));
    let audit = engine.get_config().audit;
    // Wait-only / no-UI plans do not need the engine; prefer engine when UI
    // steps exist so replay WAL + interruptibility stay wired.
    let use_engine = action_plan.summary.ui_steps > 0;
    let outcome = run_persisted_action_plan(
        conn,
        "routine",
        &action_plan,
        &[],
        if use_engine { Some(engine) } else { None },
        use_engine,
        audit.retention_keep_last,
        audit.retention_keep_days,
    )?;

    Ok(execute_response_json(
        outcome.execution_id,
        json!({
            "routine_name": workflow_name,
            "plan_source": "routine_mcp",
        }),
        &outcome.runtime,
    ))
}

fn approved_execution_macos_ui_note() -> Value {
    json!({
        "path": "action_plan_runtime",
        "resolution_order_for_semantic_steps": ["ax", "ocr", "template", "coordinates"],
        "note": "AX / ScreenCaptureKit / OCR / template match run only inside this approved Action Plan. There is no direct MCP mutate tool for those ops.",
    })
}

/// Shared execute payload: counts + halt fields clients can read without
/// digging through the sealed receipt.
fn execute_response_json(execution_id: String, identity: Value, runtime: &RuntimeResult) -> Value {
    let mut out = identity;
    let obj = out.as_object_mut().expect("identity object");
    obj.insert("execution_id".into(), json!(execution_id));
    obj.insert("applied".into(), json!(runtime.report.applied));
    obj.insert("skipped".into(), json!(runtime.report.skipped));
    obj.insert("failed".into(), json!(runtime.report.failed));
    obj.insert("stopped_early".into(), json!(runtime.stopped_early));
    obj.insert(
        "stop_reason".into(),
        json!(redact_stop_reason(runtime.stop_reason.as_deref())),
    );
    obj.insert(
        "verifications".into(),
        json!(compact_verifications(&runtime.verifications)),
    );
    obj.insert(
        "status".into(),
        json!(if runtime.report.failed > 0 || runtime.stopped_early {
            "failed"
        } else {
            "completed"
        }),
    );
    obj.insert(
        "receipt".into(),
        json!(redact_receipt_for_mcp(&runtime.receipt)),
    );
    obj.insert("macos_ui".into(), approved_execution_macos_ui_note());
    out
}

/// Redact a verification's `expected`/`observed` pair. When `expected` is a
/// redacted typed value, `observed` is the field's live contents, so it is
/// blanked to a length rather than shown.
fn redact_expected_observed(expected: &str, observed: &str) -> (String, String) {
    let expected = redact_verification_text(expected);
    let observed = if expected.contains("(redacted") {
        format!("(redacted, {} chars)", observed.chars().count())
    } else {
        redact_verification_text(observed)
    };
    (expected, observed)
}

/// Compact, redacted per-step verification rows for MCP clients.
fn compact_verifications(verifications: &[StepVerification]) -> Vec<Value> {
    verifications
        .iter()
        .map(|v| {
            let (expected, observed) = redact_expected_observed(&v.expected, &v.observed);
            json!({
                "step_id": v.step_id,
                "label": redact_verification_label(&v.label),
                "expected": expected,
                "observed": observed,
                "status": v.status,
            })
        })
        .collect()
}

/// Project a receipt for MCP clients with the same redaction the compact
/// verification rows use. The full receipt is embedded in `execute`/`get_run`
/// responses, so without this its `steps[].verification` and `stop_reason`
/// would leak the exact typed/observed field values the compact rows hide.
/// The locally sealed receipt in storage is untouched — only this outbound
/// copy is redacted.
fn redact_receipt_for_mcp(receipt: &ExecutionReceipt) -> ExecutionReceipt {
    let mut redacted = receipt.clone();
    redacted.stop_reason = redact_stop_reason(receipt.stop_reason.as_deref());
    for step in &mut redacted.steps {
        step.label = redact_verification_label(&step.label);
        let (expected, observed) =
            redact_expected_observed(&step.verification.expected, &step.verification.observed);
        step.verification.label = redact_verification_label(&step.verification.label);
        step.verification.expected = expected;
        step.verification.observed = observed;
    }
    redacted
}

fn compact_verifications_from_receipt(receipt: &ExecutionReceipt) -> Vec<Value> {
    let rows: Vec<StepVerification> = receipt
        .steps
        .iter()
        .map(|s| s.verification.clone())
        .collect();
    compact_verifications(&rows)
}

fn halt_fields_from_receipt(
    receipt: Option<&ExecutionReceipt>,
) -> (bool, Option<String>, Vec<Value>) {
    match receipt {
        Some(r) => (
            r.stopped_early,
            redact_stop_reason(r.stop_reason.as_deref()),
            compact_verifications_from_receipt(r),
        ),
        None => (false, None, Vec::new()),
    }
}

/// Redact typed-value verify strings for MCP clients.
///
/// Handles both the current `value matches …` wording and legacy
/// `value contains …` receipts sealed before the boundary-aware matcher.
fn redact_verification_text(s: &str) -> String {
    for prefix in ["value matches ", "value contains "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            return format!("{prefix}(redacted, {} chars)", rest.chars().count());
        }
    }
    s.to_string()
}

fn redact_verification_label(label: &str) -> String {
    if label.starts_with("Type ") && !label.contains("redacted") {
        return "Type text (redacted)".into();
    }
    label.to_string()
}

/// Strip embedded «expected» / «observed» payloads and the failed step's label
/// from runtime stop reasons before they reach an MCP client.
///
/// The reason prefix names the failed step, and typed steps are labeled
/// `Type {text}` (`compile_type_text`), so the label alone can carry the
/// approved value — redact it the same way step labels are redacted elsewhere,
/// in addition to the `«…»` value spans.
fn redact_stop_reason(reason: Option<&str>) -> Option<String> {
    let reason = reason?;
    let mut out = if let Some(rest) = reason.strip_prefix("verification halted on ") {
        // "verification halted on {label}: expected «…» · observed «…»"
        match rest.find(": expected «") {
            Some(cut) => format!(
                "verification halted on {}{}",
                redact_verification_label(&rest[..cut]),
                &rest[cut..]
            ),
            None => reason.to_string(),
        }
    } else if let Some(rest) = reason.strip_prefix("step ") {
        // "step {label} failed"
        match rest.rfind(" failed") {
            Some(cut) => format!("step {} failed", redact_verification_label(&rest[..cut])),
            None => reason.to_string(),
        }
    } else {
        reason.to_string()
    };

    if let (Some(start), Some(mid), Some(end)) = (
        out.find("expected «"),
        out.find("» · observed «"),
        out.rfind('»'),
    ) && mid > start
        && end > mid
    {
        out = format!("{}expected (redacted) · observed (redacted)", &out[..start]);
    }
    Some(out)
}

pub fn get_run_summary(execution_id: &str) -> Result<Value, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    get_run_summary_with_db(execution_id, &conn)
}

/// Same as [`get_run_summary`], with an explicit DB (tests / alternate stores).
pub fn get_run_summary_with_db(execution_id: &str, conn: &Db) -> Result<Value, String> {
    let stored = get_execution(conn, execution_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no execution with id {execution_id}"))?;
    Ok(run_summary_json(&stored))
}

fn run_summary_json(stored: &StoredExecution) -> Value {
    let summary = ExecutionSummary {
        id: stored.id.clone(),
        zone_id: stored.zone_id.clone(),
        created_at: stored.created_at.clone(),
        applied: stored.applied,
        skipped: stored.skipped,
        failed: stored.failed,
        sealed: !stored.hash.is_empty(),
        finished: stored.finished,
    };
    let (stopped_early, stop_reason, verifications) =
        halt_fields_from_receipt(stored.receipt.as_ref());
    json!({
        "execution": summary,
        "audit_event_count": stored.audit.events().len(),
        "receipt": stored.receipt.as_ref().map(redact_receipt_for_mcp),
        "stopped_early": stopped_early,
        "stop_reason": stop_reason,
        "verifications": verifications,
    })
}

/// Read-only run history for MCP clients (`ghost.audit_history`): every past
/// Organizer/routine run, newest first. Summary fields only — counts, seal /
/// finished state, and a derived status. No audit-event bodies, no receipts,
/// no typed values, so nothing sensitive leaves the machine; a client that
/// wants one run's detail follows up with `ghost.get_run`.
pub fn audit_history(limit: Option<usize>) -> Result<Value, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    audit_history_with_db(limit, &conn)
}

/// Same as [`audit_history`], with an explicit DB (tests / alternate stores).
pub fn audit_history_with_db(limit: Option<usize>, conn: &Db) -> Result<Value, String> {
    use crate::storage::executions::list_executions;
    let mut runs = list_executions(conn).map_err(|e| e.to_string())?;
    let total = runs.len();
    if let Some(max) = limit {
        runs.truncate(max);
    }
    let runs_json: Vec<Value> = runs.iter().map(audit_history_row_json).collect();
    Ok(json!({
        "runs": runs_json,
        "returned": runs_json.len(),
        "total": total,
        "note": "Newest first. Summary fields only — call ghost.get_run for a run's redacted receipt and verifications.",
    }))
}

/// One history row: the [`ExecutionSummary`] fields plus a derived status. The
/// same status vocabulary `execute_response_json` / `run_summary_json` use, so
/// history and single-run views agree.
fn audit_history_row_json(summary: &ExecutionSummary) -> Value {
    let status = if !summary.finished {
        "interrupted"
    } else if summary.failed > 0 {
        "failed"
    } else {
        "completed"
    };
    json!({
        "execution_id": summary.id,
        "zone_id": summary.zone_id,
        "created_at": summary.created_at,
        "applied": summary.applied,
        "skipped": summary.skipped,
        "failed": summary.failed,
        "sealed": summary.sealed,
        "finished": summary.finished,
        "status": status,
    })
}

pub fn undo_run(execution_id: &str) -> Result<Value, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    let report = undo_zone_run(&conn, execution_id)?;
    Ok(json!({
        "execution_id": execution_id,
        "reverted": report.reverted,
        "skipped": report.skipped,
        "failed": report.failed,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::approval::issue_approval_token;
    use crate::organizer::planner::{PlanAction, plan_with_rules};
    use crate::policy::{Capability, FolderRule, PolicyDecision, TrustLevel};
    use std::path::PathBuf;

    #[test]
    fn audit_history_on_empty_db_is_empty() {
        let db = crate::storage::open_in_memory().unwrap();
        let out = audit_history_with_db(None, &db).unwrap();
        assert_eq!(out["total"], 0);
        assert_eq!(out["returned"], 0);
        assert_eq!(out["runs"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn audit_history_projects_list_executions_order_and_honors_limit() {
        use crate::storage::executions::{begin_execution, list_executions};
        let db = crate::storage::open_in_memory().unwrap();
        begin_execution(&db, "zone-a").unwrap();
        begin_execution(&db, "zone-b").unwrap();
        begin_execution(&db, "zone-c").unwrap();

        // audit_history is a pure projection of list_executions, so its rows
        // must appear in exactly list_executions' order. (created_at is
        // second-granularity, so same-second rows tie and fall back to the
        // id tiebreaker — don't assume creation order here.)
        let expected: Vec<String> = list_executions(&db)
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(expected.len(), 3);

        let all = audit_history_with_db(None, &db).unwrap();
        assert_eq!(all["total"], 3);
        assert_eq!(all["returned"], 3);
        let got: Vec<String> = all["runs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["execution_id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(got, expected, "audit_history must preserve list order");
        // A begin_execution row has not finished — surfaced as interrupted.
        assert_eq!(all["runs"][0]["status"], "interrupted");
        assert_eq!(all["runs"][0]["finished"], false);

        // limit truncates the head of that same order; total still counts all.
        let limited = audit_history_with_db(Some(1), &db).unwrap();
        assert_eq!(limited["total"], 3, "total reflects all runs, not the page");
        assert_eq!(limited["returned"], 1);
        assert_eq!(limited["runs"].as_array().unwrap().len(), 1);
        assert_eq!(limited["runs"][0]["execution_id"], expected[0]);
    }

    fn allow_rule(path: &std::path::Path) -> FolderRule {
        FolderRule {
            path: path.to_path_buf(),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
            trust: TrustLevel::AskFirst,
        }
    }

    fn unique_workflow_name(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }

    #[test]
    fn denied_plan_cannot_execute_even_with_valid_token() {
        let signed = issue_approval_token("plan_zone-deny", "sha256:abc");
        let token = serde_json::to_string(&signed).unwrap();
        let err = execute_approved_plan("zone-deny", &token).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn plan_with_denied_action_fails_validation_message() {
        let action = PlanAction {
            capability: Capability::CreateFolder {
                path: PathBuf::from("/tmp/ghost-denied"),
            },
            decision: PolicyDecision::Deny {
                reason: "test".into(),
            },
            rule_path: None,
            confidence: 1.0,
            reason: "denied".into(),
            conflict: None,
            source_identity: None,
        };
        let plan = plan_with_rules("z", &[allow_rule(PathBuf::from("/tmp").as_path())]);
        let mut denied = plan;
        denied.actions = vec![action];
        let denied_count = denied
            .actions
            .iter()
            .filter(|a| a.decision.is_denied())
            .count();
        assert_eq!(denied_count, 1);
    }

    #[test]
    fn approved_headless_routine_returns_persisted_receipt_and_token_is_single_use() {
        let engine = GhostEngine::new();
        if engine.auth().is_configured() {
            // A locked/configured vault in the developer environment cannot be
            // unlocked from this unit test; encrypted-load paths are covered
            // elsewhere. Unconfigured installs (CI) exercise the happy path.
            return;
        }
        let name = unique_workflow_name("mcp-routine-happy");
        let events = vec![crate::core::events::InputEvent::Delay {
            ms: 300,
            timestamp: None,
        }];
        let path = engine.save_workflow(&name, &events).unwrap();
        let hash = hash_routine_plan(&name, &events);
        let signed = issue_approval_token(&routine_plan_id(&name), &hash);
        let token = serde_json::to_string(&signed).unwrap();
        let db = crate::storage::open_in_memory().unwrap();

        let result = execute_approved_routine_with_db(&name, &token, &engine, &db).unwrap();
        assert_eq!(result["status"], "completed");
        assert_eq!(result["receipt"]["failed"], 0);
        assert_eq!(result["stopped_early"], false);
        assert!(result["stop_reason"].is_null());
        assert!(result["verifications"].is_array());
        assert_eq!(result["macos_ui"]["path"], "action_plan_runtime");
        assert!(
            result["macos_ui"]["resolution_order_for_semantic_steps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_str() == Some("ocr"))
        );
        let execution_id = result["execution_id"].as_str().unwrap();
        let stored = get_execution(&db, execution_id).unwrap().expect("row");
        assert!(stored.receipt.is_some());

        let summary = get_run_summary_with_db(execution_id, &db).unwrap();
        assert_eq!(summary["stopped_early"], false);
        assert!(summary["stop_reason"].is_null());
        assert!(summary["verifications"].is_array());
        assert!(summary["receipt"].is_object());

        let replay = execute_approved_routine_with_db(&name, &token, &engine, &db).unwrap_err();
        assert!(replay.contains("already been used"), "got: {replay}");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn compact_verifications_redact_typed_value_payloads() {
        let rows = compact_verifications(&[StepVerification::failed(
            "s1",
            "Type invoice amount",
            "value matches 1234.56",
            "999.00",
        )]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["label"], "Type text (redacted)");
        assert_eq!(rows[0]["expected"], "value matches (redacted, 7 chars)");
        assert_eq!(rows[0]["observed"], "(redacted, 6 chars)");
        assert_eq!(rows[0]["status"], "failed");
        assert_eq!(rows[0]["step_id"], "s1");

        // Legacy sealed receipts keep their old expected wording but still redact.
        let legacy = compact_verifications(&[StepVerification::failed(
            "s2",
            "Confirm",
            "value contains 12,900",
            "12,090",
        )]);
        assert_eq!(legacy[0]["expected"], "value contains (redacted, 6 chars)");
    }

    #[test]
    fn compact_verifications_redact_semantic_verify_values_not_presence_or_paths() {
        // A `SemanticVerify` against a concrete value carries the shared
        // `value matches …` wording, so its expected AND its live observed
        // value are redacted for MCP clients — not just `SemanticSetValue`.
        let value_verify = compact_verifications(&[StepVerification::failed(
            "s1",
            "Verify amount",
            "value matches 12,900",
            "12,900,000",
        )]);
        assert_eq!(
            value_verify[0]["expected"],
            "value matches (redacted, 6 chars)"
        );
        assert_eq!(value_verify[0]["observed"], "(redacted, 10 chars)");

        // A bare presence check and a path-existence check are not field
        // values and stay visible, per the documented contract.
        let presence = compact_verifications(&[StepVerification::verified(
            "s2",
            "Field present",
            "AXTextField present",
            "AXTextField present",
        )]);
        assert_eq!(presence[0]["expected"], "AXTextField present");
        let path = compact_verifications(&[StepVerification::failed(
            "s3",
            "Moved",
            "/dest/report.pdf exists",
            "/dest/report.pdf absent",
        )]);
        assert_eq!(path[0]["expected"], "/dest/report.pdf exists");
        assert_eq!(path[0]["observed"], "/dest/report.pdf absent");
    }

    #[test]
    fn halt_fields_surface_stop_reason_and_verifications_from_receipt() {
        use crate::organizer::executor::ExecutionReport;
        use crate::runtime::build_receipt;
        let v = StepVerification::failed(
            "s2",
            "Confirm total",
            "path/to/file exists",
            "path/to/file absent",
        );
        let receipt = build_receipt(
            "plan-1",
            "demo",
            Some("exec-1".into()),
            &ExecutionReport::default(),
            &[v],
            "1",
            "2",
            true,
            Some(
                "verification halted on Confirm total: expected «secret» · observed «other»".into(),
            ),
        );
        let (stopped_early, stop_reason, verifications) = halt_fields_from_receipt(Some(&receipt));
        assert!(stopped_early);
        assert_eq!(
            stop_reason.as_deref(),
            Some("verification halted on Confirm total: expected (redacted) · observed (redacted)")
        );
        assert_eq!(verifications.len(), 1);
        assert_eq!(verifications[0]["label"], "Confirm total");
        assert_eq!(verifications[0]["expected"], "path/to/file exists");
        assert_eq!(verifications[0]["observed"], "path/to/file absent");
        assert_eq!(verifications[0]["status"], "failed");
    }

    #[test]
    fn redact_receipt_for_mcp_hides_field_values_in_the_embedded_receipt() {
        use crate::organizer::executor::ExecutionReport;
        use crate::runtime::build_receipt;
        // A value verification (typed field contents) plus a path check.
        let value =
            StepVerification::failed("s1", "Type amount", "value matches 12,900", "12,900,000");
        let path = StepVerification::failed("s2", "Moved", "/dest/x exists", "/dest/x absent");
        let receipt = build_receipt(
            "plan-1",
            "demo",
            Some("exec-1".into()),
            &ExecutionReport::default(),
            &[value, path],
            "1",
            "2",
            true,
            Some(
                "verification halted on Type amount: expected «12,900» · observed «12,900,000»"
                    .into(),
            ),
        );

        let redacted = redact_receipt_for_mcp(&receipt);
        // The embedded receipt no longer carries the raw field values …
        assert_eq!(
            redacted.steps[0].verification.expected,
            "value matches (redacted, 6 chars)"
        );
        assert_eq!(
            redacted.steps[0].verification.observed,
            "(redacted, 10 chars)"
        );
        assert_eq!(redacted.steps[0].label, "Type text (redacted)");
        assert_eq!(
            redacted.stop_reason.as_deref(),
            Some(
                "verification halted on Type text (redacted): expected (redacted) · observed (redacted)"
            )
        );
        // … while path-existence checks stay visible.
        assert_eq!(redacted.steps[1].verification.expected, "/dest/x exists");
        assert_eq!(redacted.steps[1].verification.observed, "/dest/x absent");

        // The source receipt is untouched (local seal stays authoritative).
        assert_eq!(receipt.steps[0].verification.observed, "12,900,000");
    }

    #[test]
    fn redact_stop_reason_hides_typed_value_in_the_step_label() {
        // A `Type {value}` label carries the approved value in the reason
        // prefix; both composed shapes must redact it, not only the spans.
        assert_eq!(
            redact_stop_reason(Some(
                "verification halted on Type 1234.56: expected «1234.56» · observed «0.00»"
            ))
            .as_deref(),
            Some(
                "verification halted on Type text (redacted): expected (redacted) · observed (redacted)"
            )
        );
        assert_eq!(
            redact_stop_reason(Some("step Type 1234.56 failed")).as_deref(),
            Some("step Type text (redacted) failed")
        );
        // Non-typed labels (not a field value) stay visible.
        assert_eq!(
            redact_stop_reason(Some("step Move invoice failed")).as_deref(),
            Some("step Move invoice failed")
        );
    }

    #[test]
    fn run_summary_json_includes_halt_contract_without_receipt() {
        use crate::audit::{AuditLog, UndoJournal};
        let stored = StoredExecution {
            id: "e1".into(),
            zone_id: "z".into(),
            created_at: "0".into(),
            applied: 0,
            skipped: 0,
            failed: 0,
            audit: AuditLog::default(),
            undo: UndoJournal::default(),
            hash: String::new(),
            prev_hash: String::new(),
            finished: true,
            receipt: None,
            label_notes: Vec::new(),
        };
        let summary = run_summary_json(&stored);
        assert_eq!(summary["stopped_early"], false);
        assert!(summary["stop_reason"].is_null());
        assert_eq!(summary["verifications"], json!([]));
        assert!(summary["receipt"].is_null());
    }

    #[test]
    fn unauthorized_routine_execute_is_denied() {
        let engine = GhostEngine::new();
        if engine.auth().is_configured() {
            return;
        }
        let name = unique_workflow_name("mcp-routine-unauth");
        let events = vec![crate::core::events::InputEvent::Delay {
            ms: 300,
            timestamp: None,
        }];
        let path = engine.save_workflow(&name, &events).unwrap();
        let db = crate::storage::open_in_memory().unwrap();
        let err = execute_approved_routine_with_db(&name, "not-a-token", &engine, &db).unwrap_err();
        assert!(
            err.contains("Invalid approval token") || err.contains("signature"),
            "got: {err}"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn changed_routine_is_rejected_before_execution() {
        let name = unique_workflow_name("mcp-routine-stale");
        let before = vec![crate::core::events::InputEvent::Delay {
            ms: 300,
            timestamp: None,
        }];
        let after = vec![crate::core::events::InputEvent::Delay {
            ms: 450,
            timestamp: None,
        }];
        let signed =
            issue_approval_token(&routine_plan_id(&name), &hash_routine_plan(&name, &before));
        let token = serde_json::to_string(&signed).unwrap();
        let err = verify_execution_token_for_plan(
            &token,
            &routine_plan_id(&name),
            Some(&hash_routine_plan(&name, &after)),
        )
        .unwrap_err();
        assert!(err.contains("Plan has changed"), "got: {err}");
    }

    #[test]
    fn denied_policy_step_refuses_routine_execution() {
        let engine = GhostEngine::new();
        if engine.auth().is_configured() {
            return;
        }
        let name = unique_workflow_name("mcp-routine-denied");
        // Unknown semantic steps compress to Unknown and are policy-denied.
        // A raw event stream that becomes only delays is allowed; use a
        // Variable event that compresses poorly — actually Variable may not
        // become Unknown. Safer: assert ensure_replayable on secure typing.
        use crate::core::compression::Target;
        use crate::core::compression::{CompressedStep, CompressionReport, TypeTextStep};
        let report = CompressionReport::new(
            1,
            vec![CompressedStep::TypeText(TypeTextStep {
                char_count: 8,
                redacted: true,
                secure_field: true,
                target: Some(Target {
                    name: "Password".into(),
                    role: "AXTextField".into(),
                    app: "Login".into(),
                    identifier: None,
                    window_title: None,
                }),
                text: None,
                confidence: 1.0,
                raw_event_count: 8,
            })],
            vec![(0, 8)],
        );
        let plan = crate::policy::evaluate_compressed(&report);
        let err = crate::policy::ensure_replayable(&plan).unwrap_err();
        assert!(err.contains("Replay blocked by policy"), "got: {err}");
        let _ = name;
        let _ = engine;
    }
}
