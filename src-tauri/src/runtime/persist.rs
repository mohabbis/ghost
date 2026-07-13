//! Write-ahead persisted execution — shared by Organizer, MCP, and IPC commands.

use super::execute::{execute_action_plan_with_progress, RuntimeResult};
use crate::action_plan::ActionPlan;
use crate::engine::GhostEngine;
use crate::policy::FolderRule;
use crate::storage::executions::{
    begin_execution, finish_execution, prune_executions, update_execution_progress,
};
use crate::storage::milestones::{record_milestone, Milestone};
use crate::storage::Db;

#[derive(Debug, Clone)]
pub struct PersistedRunOutcome {
    pub execution_id: String,
    pub runtime: RuntimeResult,
}

/// Execute an approved plan with WAL persistence and optional retention pruning.
#[allow(clippy::too_many_arguments)]
pub fn run_persisted_action_plan(
    conn: &Db,
    zone_id: &str,
    action_plan: &ActionPlan,
    rules: &[FolderRule],
    engine: Option<&GhostEngine>,
    use_engine: bool,
    retention_keep_last: Option<usize>,
    retention_keep_days: Option<u64>,
) -> Result<PersistedRunOutcome, String> {
    let execution_id = begin_execution(conn, zone_id).map_err(|e| e.to_string())?;
    let engine_ref = if use_engine { engine } else { None };

    let runtime = execute_action_plan_with_progress(
        action_plan,
        rules,
        engine_ref,
        Some(execution_id.clone()),
        |partial| {
            let _ = update_execution_progress(conn, &execution_id, partial);
        },
    );

    finish_execution(
        conn,
        &execution_id,
        zone_id,
        &runtime.report,
        Some(&runtime.receipt),
    )
    .map_err(|e| e.to_string())?;
    let _ = record_milestone(conn, Milestone::FirstRun);
    let _ = prune_executions(conn, retention_keep_last, retention_keep_days);

    Ok(PersistedRunOutcome {
        execution_id,
        runtime,
    })
}
