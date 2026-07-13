//! Ghost 2.0 unified action plan commands — one pipeline for all intent sources.

use crate::action_plan::{
    build_invoice_demo, from_compression_report, from_organizer_plan, ActionPlan, PlanSource,
};
use crate::core::compression::compress;
use crate::core::events::InputEvent;
use crate::engine::GhostEngine;
use crate::organizer::pipeline::undo_zone_run;
use crate::organizer::planner::plan_zone;
use crate::policy::{ensure_replayable, evaluate_compressed, fingerprint_events};
use crate::runtime::{run_persisted_action_plan, ExecutionReceipt};
use crate::storage::executions::get_execution;
use crate::storage::zones::list_folder_rules;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlanExecuteResult {
    pub execution_id: Option<String>,
    pub report_summary: ExecuteSummary,
    pub receipt: ExecutionReceipt,
    pub stopped_early: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteSummary {
    pub applied: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// Compile a reviewable action plan from an Organizer zone (read-only).
#[tauri::command]
pub fn action_plan_from_zone(zone_id: String) -> Result<ActionPlan, String> {
    let conn = crate::storage::open_default().map_err(|e| e.to_string())?;
    let plan = plan_zone(&conn, &zone_id).map_err(|e| e.to_string())?;
    Ok(from_organizer_plan(&plan))
}

/// Compile a reviewable action plan from recorded routine events (read-only).
#[tauri::command]
pub fn action_plan_from_events(events: Vec<InputEvent>) -> Result<ActionPlan, String> {
    let report = compress(&events);
    Ok(from_compression_report(&report, &events, None))
}

/// Build the Ghost 2.0 invoice demonstration workflow (read-only).
#[tauri::command]
pub fn action_plan_demo(downloads: String, finance_root: String) -> Result<ActionPlan, String> {
    build_invoice_demo(
        PathBuf::from(downloads).as_path(),
        PathBuf::from(finance_root).as_path(),
    )
}

/// Execute through the canonical runtime. Recompiles server-side from `source` — never trusts client steps for Organizer.
#[tauri::command]
pub fn execute_action_plan(
    source: PlanSource,
    events: Option<Vec<InputEvent>>,
    downloads: Option<String>,
    finance_root: Option<String>,
    engine: State<GhostEngine>,
) -> Result<ActionPlanExecuteResult, String> {
    super::auth::require_unlocked(&engine)?;
    let action_plan = recompile_plan(&source, events.as_deref(), downloads, finance_root)?;
    run_action_plan(&action_plan, Some(&engine), needs_engine(&action_plan))
}

/// Execute a routine action plan after policy approval.
#[tauri::command]
pub fn execute_routine_action_plan(
    events: Vec<InputEvent>,
    workflow_name: Option<String>,
    engine: State<GhostEngine>,
) -> Result<ActionPlanExecuteResult, String> {
    let report = compress(&events);
    let policy_plan = evaluate_compressed(&report);
    ensure_replayable(&policy_plan)?;
    engine.consume_routine_approval(&fingerprint_events(&events))?;

    let mut action_plan = from_compression_report(&report, &events, workflow_name);
    if let PlanSource::Routine {
        ref mut workflow_name,
        ref mut fingerprint,
        ..
    } = action_plan.source
    {
        *fingerprint = Some(fingerprint_events(&events));
        let _ = workflow_name;
    }
    run_action_plan(&action_plan, Some(&engine), true)
}

/// Fetch receipt for a persisted Organizer execution.
#[tauri::command]
pub fn get_execution_receipt(execution_id: String) -> Result<ExecutionReceipt, String> {
    let conn = crate::storage::open_default().map_err(|e| e.to_string())?;
    let stored = get_execution(&conn, &execution_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no execution with id {execution_id}"))?;

    if let Some(receipt) = stored.receipt {
        return Ok(receipt);
    }

    let verifications: Vec<crate::runtime::StepVerification> = stored
        .audit
        .events()
        .iter()
        .enumerate()
        .map(|(i, ev)| {
            let label = format!("{:?}", ev.capability);
            crate::runtime::StepVerification::not_applicable(
                &format!("audit-{i}"),
                &label,
                &format!("{:?}", ev.outcome),
            )
        })
        .collect();

    let report = crate::organizer::executor::ExecutionReport {
        applied: stored.applied,
        skipped: stored.skipped,
        failed: stored.failed,
        audit: stored.audit.clone(),
        undo: stored.undo.clone(),
    };

    Ok(crate::runtime::build_receipt(
        &execution_id,
        &stored.zone_id,
        Some(execution_id.clone()),
        &report,
        &verifications,
        &stored.created_at,
        &stored.created_at,
        false,
        None,
    ))
}

fn recompile_plan(
    source: &PlanSource,
    events: Option<&[InputEvent]>,
    downloads: Option<String>,
    finance_root: Option<String>,
) -> Result<ActionPlan, String> {
    match source {
        PlanSource::Organizer { zone_id } => {
            let conn = crate::storage::open_default().map_err(|e| e.to_string())?;
            let plan = plan_zone(&conn, zone_id).map_err(|e| e.to_string())?;
            Ok(from_organizer_plan(&plan))
        }
        PlanSource::Routine { workflow_name, .. } => {
            let events = events.ok_or("routine execution requires events".to_string())?;
            let report = compress(events);
            Ok(from_compression_report(
                &report,
                events,
                workflow_name.clone(),
            ))
        }
        PlanSource::Mcp { zone_id } => {
            let conn = crate::storage::open_default().map_err(|e| e.to_string())?;
            let plan = plan_zone(&conn, zone_id).map_err(|e| e.to_string())?;
            Ok(from_organizer_plan(&plan))
        }
        PlanSource::Demo => {
            let downloads = downloads.ok_or("demo requires downloads path".to_string())?;
            let finance_root = finance_root.ok_or("demo requires finance_root path".to_string())?;
            build_invoice_demo(
                PathBuf::from(downloads).as_path(),
                PathBuf::from(finance_root).as_path(),
            )
        }
        PlanSource::Workflow { name } => Err(format!(
            "workflow source '{name}' must be compiled before execute"
        )),
    }
}

fn demo_rules_for_plan(plan: &ActionPlan) -> Vec<crate::policy::FolderRule> {
    use crate::action_plan::types::ActionKind;
    use crate::policy::{FolderRule, TrustLevel};
    use std::collections::HashSet;
    let mut paths = HashSet::new();
    for step in &plan.steps {
        match &step.kind {
            ActionKind::CreateFolder { path } | ActionKind::VerifyPath { path, .. } => {
                paths.insert(path.clone());
                if let Some(parent) = path.parent() {
                    paths.insert(parent.to_path_buf());
                }
            }
            ActionKind::MoveFile { from, to } | ActionKind::RenameFile { from, to } => {
                for p in [from, to] {
                    paths.insert(p.clone());
                    if let Some(parent) = p.parent() {
                        paths.insert(parent.to_path_buf());
                    }
                }
            }
            _ => {}
        }
    }
    paths
        .into_iter()
        .map(|path| FolderRule {
            path,
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
            trust: TrustLevel::Automate,
        })
        .collect()
}

fn needs_engine(plan: &ActionPlan) -> bool {
    plan.summary.ui_steps > 0
}

fn run_action_plan(
    action_plan: &ActionPlan,
    engine: Option<&GhostEngine>,
    use_engine: bool,
) -> Result<ActionPlanExecuteResult, String> {
    let conn = crate::storage::open_default().map_err(|e| e.to_string())?;
    let audit = engine.map(|e| e.get_config().audit);
    let retention_keep_last = audit.as_ref().and_then(|a| a.retention_keep_last);
    let retention_keep_days = audit.as_ref().and_then(|a| a.retention_keep_days);
    let zone_id = match &action_plan.source {
        PlanSource::Organizer { zone_id } | PlanSource::Mcp { zone_id } => zone_id.clone(),
        PlanSource::Routine { .. } => "routine".into(),
        PlanSource::Demo => "demo".into(),
        PlanSource::Workflow { name } => name.clone(),
    };

    let rules = match &action_plan.source {
        PlanSource::Routine { .. } => vec![],
        PlanSource::Demo => demo_rules_for_plan(action_plan),
        PlanSource::Organizer { zone_id } | PlanSource::Mcp { zone_id } => {
            list_folder_rules(&conn, zone_id).map_err(|e| e.to_string())?
        }
        PlanSource::Workflow { .. } => vec![],
    };

    let outcome = run_persisted_action_plan(
        &conn,
        &zone_id,
        action_plan,
        &rules,
        if use_engine { engine } else { None },
        use_engine,
        retention_keep_last,
        retention_keep_days,
    )?;

    Ok(ActionPlanExecuteResult {
        execution_id: Some(outcome.execution_id),
        report_summary: ExecuteSummary {
            applied: outcome.runtime.report.applied,
            skipped: outcome.runtime.report.skipped,
            failed: outcome.runtime.report.failed,
        },
        receipt: outcome.runtime.receipt,
        stopped_early: outcome.runtime.stopped_early,
    })
}

/// Undo a unified execution (Organizer / demo with WAL).
#[tauri::command]
pub fn undo_action_plan_execution(
    execution_id: String,
) -> Result<crate::organizer::undo::UndoReport, String> {
    let conn = crate::storage::open_default().map_err(|e| e.to_string())?;
    undo_zone_run(&conn, &execution_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::testutil::tempdir;
    use std::fs;

    #[test]
    fn demo_plan_compiles_and_serializes() {
        let tmp = tempdir();
        let downloads = tmp.path().join("dl");
        let finance = tmp.path().join("fin");
        fs::create_dir_all(&downloads).unwrap();
        fs::write(downloads.join("invoice.pdf"), b"x").unwrap();
        let plan = build_invoice_demo(&downloads, &finance).unwrap();
        let json = serde_json::to_string(&plan).unwrap();
        let back: ActionPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back.steps.len(), plan.steps.len());
    }
}
