//! Headless MCP execution helpers — reuse the Organizer trust pipeline.

use crate::mcp::approval::verify_execution_token_with_hash;
use crate::mcp::plan_hash::hash_organizer_plan;
use crate::organizer::pipeline::{execute_zone, undo_zone_run};
use crate::organizer::planner::plan_zone;
use crate::storage::executions::{get_execution, ExecutionSummary};
use crate::storage::open_default;
use serde_json::{json, Value};

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

    let outcome = execute_zone(&conn, zone_id, None, None)?;
    Ok(json!({
        "execution_id": outcome.execution_id,
        "zone_id": zone_id,
        "applied": outcome.report.applied,
        "skipped": outcome.report.skipped,
        "failed": outcome.report.failed,
        "status": if outcome.report.failed > 0 { "failed" } else { "completed" },
    }))
}

pub fn get_run_summary(execution_id: &str) -> Result<Value, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    let stored = get_execution(&conn, execution_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no execution with id {execution_id}"))?;
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
    Ok(json!({
        "execution": summary,
        "audit_event_count": stored.audit.events().len(),
    }))
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
