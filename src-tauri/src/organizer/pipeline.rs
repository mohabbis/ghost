//! Shared Organizer run/undo pipeline for Tauri commands and the headless MCP server.

use crate::action_plan::{from_organizer_plan_with_source, PlanSource};
use crate::organizer::executor::ExecutionReport;
use crate::organizer::planner::plan_zone;
use crate::organizer::undo::{revert, UndoReport};
use crate::runtime::run_persisted_action_plan;
use crate::storage::executions::{get_execution, mark_execution_finished};
use crate::storage::milestones::{record_milestone, Milestone};
use crate::storage::zones::list_folder_rules;
use crate::storage::Db;

#[derive(Debug, Clone)]
pub struct ZoneRunOutcome {
    pub execution_id: String,
    pub report: ExecutionReport,
    pub receipt: crate::runtime::ExecutionReceipt,
}

/// Re-plan server-side and execute through the canonical ActionPlan runtime.
pub fn execute_zone(
    conn: &Db,
    zone_id: &str,
    retention_keep_last: Option<usize>,
    retention_keep_days: Option<u64>,
) -> Result<ZoneRunOutcome, String> {
    execute_zone_with_source(
        conn,
        zone_id,
        PlanSource::Organizer {
            zone_id: zone_id.into(),
        },
        retention_keep_last,
        retention_keep_days,
    )
}

/// Like [`execute_zone`], but records the intent source on the compiled plan (e.g. MCP).
pub fn execute_zone_with_source(
    conn: &Db,
    zone_id: &str,
    source: PlanSource,
    retention_keep_last: Option<usize>,
    retention_keep_days: Option<u64>,
) -> Result<ZoneRunOutcome, String> {
    let rules = list_folder_rules(conn, zone_id).map_err(|e| e.to_string())?;
    let plan = plan_zone(conn, zone_id).map_err(|e| e.to_string())?;
    let action_plan = from_organizer_plan_with_source(&plan, source);

    let outcome = run_persisted_action_plan(
        conn,
        zone_id,
        &action_plan,
        &rules,
        None,
        false,
        retention_keep_last,
        retention_keep_days,
    )?;

    Ok(ZoneRunOutcome {
        execution_id: outcome.execution_id,
        report: outcome.runtime.report,
        receipt: outcome.runtime.receipt,
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
