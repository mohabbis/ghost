//! Shared Organizer run/undo pipeline for Tauri commands and the headless MCP server.

use crate::organizer::executor::{execute_plan_with_progress, ExecutionReport};
use crate::organizer::planner::plan_zone;
use crate::organizer::undo::{revert, UndoReport};
use crate::storage::executions::{
    begin_execution, finish_execution, get_execution, mark_execution_finished, prune_executions,
    update_execution_progress,
};
use crate::storage::milestones::{record_milestone, Milestone};
use crate::storage::zones::list_folder_rules;
use crate::storage::Db;

#[derive(Debug, Clone)]
pub struct ZoneRunOutcome {
    pub execution_id: String,
    pub report: ExecutionReport,
}

/// Re-plan server-side and execute an approved Zone — same trust path as
/// `organizer_execute`, without accepting a client-supplied plan.
pub fn execute_zone(
    conn: &Db,
    zone_id: &str,
    retention_keep_last: Option<usize>,
    retention_keep_days: Option<u64>,
) -> Result<ZoneRunOutcome, String> {
    let rules = list_folder_rules(conn, zone_id).map_err(|e| e.to_string())?;
    let plan = plan_zone(conn, zone_id).map_err(|e| e.to_string())?;

    let execution_id = begin_execution(conn, zone_id).map_err(|e| e.to_string())?;
    let report = execute_plan_with_progress(&plan, &rules, |partial| {
        let _ = update_execution_progress(conn, &execution_id, partial);
    });
    finish_execution(conn, &execution_id, zone_id, &report).map_err(|e| e.to_string())?;
    let _ = record_milestone(conn, Milestone::FirstRun);
    let _ = prune_executions(conn, retention_keep_last, retention_keep_days);

    Ok(ZoneRunOutcome {
        execution_id,
        report,
    })
}

/// Undo a persisted execution by replaying its undo journal in reverse.
pub fn undo_zone_run(conn: &Db, execution_id: &str) -> Result<UndoReport, String> {
    let stored = get_execution(conn, execution_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no execution with id {execution_id}"))?;
    let report = revert(&stored.undo);
    let _ = record_milestone(conn, Milestone::FirstUndo);
    if !stored.finished {
        let _ = mark_execution_finished(conn, execution_id);
    }
    Ok(report)
}
