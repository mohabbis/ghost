//! Ghost Organizer commands — the IPC bridge for the trust pipeline.
//!
//! These commands expose the (already complete, separately tested) Organizer
//! backend to the frontend, in the exact order the product's trust model
//! demands:
//!
//! ```text
//! Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
//! ```
//!
//! Risk classes (see `docs/command-registry.md`):
//! - `organizer_list_zones`, `organizer_plan`, `organizer_list_executions` —
//!   **safe-read**: they touch the local DB and read directory metadata, and
//!   mutate nothing on disk.
//! - `organizer_create_zone`, `organizer_add_folder_rule` — **local-mutate**
//!   (DB only): they record user-approved boundaries.
//! - `organizer_execute`, `organizer_undo` — **local-mutate** (filesystem):
//!   they move/rename files inside an approved Zone, write an audit log and undo
//!   journal, and can roll a past run back.
//!
//! A deliberate trust choice: `organizer_execute` does **not** accept a plan from
//! the frontend. It re-plans server-side from the Zone id and the executor
//! re-checks every action through `policy::evaluate`, so a stale or tampered
//! plan posted from JS can never reach the filesystem. The preview the UI shows
//! (`organizer_plan`) and the plan actually executed are produced by the same
//! deterministic backend against the same persisted rules.

use crate::organizer::executor::{execute_plan, ExecutionReport};
use crate::organizer::planner::{plan_zone, OrganizerPlan};
use crate::organizer::undo::{revert, UndoReport};
use crate::policy::{DefaultDecision, FolderRule, Zone};
use crate::storage::executions::{
    get_execution, list_executions, save_execution, ExecutionSummary,
};
use crate::storage::open_default;
use crate::storage::zones::{add_folder_rule, create_zone, list_folder_rules, list_zones};
use serde::{Deserialize, Serialize};

/// The result of executing a plan: the report plus the id under which it was
/// stored, so the UI can offer an undo for exactly this run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Id of the persisted execution (pass to `organizer_undo`).
    pub execution_id: String,
    /// Counts, audit log, and undo journal for the run.
    pub report: ExecutionReport,
}

/// List every Zone the user has approved.
///
/// Risk class: safe-read. Touches: files (DB only). No OS input, screenshots,
/// network, secrets, or app/window state.
#[tauri::command]
pub fn organizer_list_zones() -> Result<Vec<Zone>, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    list_zones(&conn).map_err(|e| e.to_string())
}

/// List the folder rules (boundaries) attached to a Zone, so the UI can show
/// exactly which folders Ghost may work in and what it is allowed to do there.
///
/// Risk class: safe-read (DB only).
#[tauri::command]
pub fn organizer_list_folder_rules(zone_id: String) -> Result<Vec<FolderRule>, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    list_folder_rules(&conn, &zone_id).map_err(|e| e.to_string())
}

/// Create a new Zone. New Zones default to `Ask` — the user still approves each
/// mutating action; this is the boundary, not a blanket allow. `rename_dated`
/// is opt-in and prefixes proposed filenames with their file month.
///
/// Risk class: local-mutate (DB only).
#[tauri::command]
pub fn organizer_create_zone(
    name: String,
    description: Option<String>,
    rename_dated: Option<bool>,
) -> Result<Zone, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    create_zone(
        &conn,
        &name,
        description.as_deref(),
        DefaultDecision::Ask,
        rename_dated.unwrap_or(false),
    )
    .map_err(|e| e.to_string())
}

/// Attach a folder rule (a path plus per-operation permissions) to a Zone.
///
/// The rule arrives as the pure `FolderRule` domain type, so the frontend names
/// exactly which operations it is granting inside this folder. Granting nothing
/// mutating (the `read_only` shape) is the safe default.
///
/// Risk class: local-mutate (DB only).
#[tauri::command]
pub fn organizer_add_folder_rule(zone_id: String, rule: FolderRule) -> Result<(), String> {
    // Ghost never grants delete through the Organizer surface (MVP never deletes,
    // and the policy engine denies it regardless). Refuse to even persist such a
    // rule so the stored boundary can't imply a capability the product won't honor.
    if rule.can_delete {
        return Err("Organizer folder rules cannot grant delete".to_string());
    }
    let conn = open_default().map_err(|e| e.to_string())?;
    add_folder_rule(&conn, &zone_id, &rule).map_err(|e| e.to_string())
}

/// Produce a reviewable plan for a Zone. **Read-only**: this scans and proposes,
/// classifies, detects conflicts, and policy-checks every action, but mutates
/// nothing. This is the preview the user approves before anything runs.
///
/// Risk class: safe-read. Touches: files (reads directory metadata; no writes).
#[tauri::command]
pub fn organizer_plan(zone_id: String) -> Result<OrganizerPlan, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    plan_zone(&conn, &zone_id).map_err(|e| e.to_string())
}

/// Execute a Zone's plan after the user approved it.
///
/// Re-plans server-side from the Zone id and runs the executor, which
/// independently re-checks policy per action, refuses to overwrite, writes undo
/// data before each mutation, and records an audit event for every action. The
/// resulting report (with its audit log and undo journal) is persisted so the
/// run can be reviewed and undone later.
///
/// Risk class: local-mutate (filesystem). Touches: files (moves/renames inside
/// the approved Zone). No OS input, screenshots, network, secrets.
#[tauri::command]
pub fn organizer_execute(zone_id: String) -> Result<ExecutionResult, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    let rules = list_folder_rules(&conn, &zone_id).map_err(|e| e.to_string())?;
    let plan = plan_zone(&conn, &zone_id).map_err(|e| e.to_string())?;
    let report = execute_plan(&plan, &rules);
    let execution_id = save_execution(&conn, &zone_id, &report).map_err(|e| e.to_string())?;
    Ok(ExecutionResult {
        execution_id,
        report,
    })
}

/// List past executions, newest first, for the history/undo view.
///
/// Risk class: safe-read (DB only).
#[tauri::command]
pub fn organizer_list_executions() -> Result<Vec<ExecutionSummary>, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    list_executions(&conn).map_err(|e| e.to_string())
}

/// Undo a past execution by replaying its stored undo journal in reverse.
///
/// The undo runner preserves the executor's safety stance: it never overwrites
/// an occupied origin and never removes a folder that is no longer empty.
///
/// Risk class: local-mutate (filesystem).
#[tauri::command]
pub fn organizer_undo(execution_id: String) -> Result<UndoReport, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    let stored = get_execution(&conn, &execution_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no execution with id {execution_id}"))?;
    Ok(revert(&stored.undo))
}
