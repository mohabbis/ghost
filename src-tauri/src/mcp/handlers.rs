//! Headless MCP tool handlers — domain logic only, no Tauri.

use crate::mcp::tools::McpToolKind;
use crate::organizer::planner::{plan_zone, OrganizerPlan};
use crate::storage::open_default;
use crate::storage::zones::list_zones;
use serde_json::{json, Value};

pub fn handle_tool(name: &str, arguments: &Value) -> Result<Value, String> {
    let kind = tool_kind_from_name(name)?;
    match kind {
        McpToolKind::Status => Ok(json!({
            "app": "ghost",
            "mcp": "local-stdio",
            "trust_pipeline": "Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo",
        })),
        McpToolKind::ListZones => {
            let db = open_default().map_err(|e| e.to_string())?;
            let zones = list_zones(&db).map_err(|e| e.to_string())?;
            Ok(json!({ "zones": zones }))
        }
        McpToolKind::ScanZone | McpToolKind::CreatePlan => {
            let zone_id = arg_string(arguments, "zone_id")?;
            let db = open_default().map_err(|e| e.to_string())?;
            let plan = plan_zone(&db, &zone_id).map_err(|e| e.to_string())?;
            Ok(plan_to_json(&plan))
        }
        McpToolKind::GetPlan => {
            let zone_id = arg_string(arguments, "zone_id")?;
            let db = open_default().map_err(|e| e.to_string())?;
            let plan = plan_zone(&db, &zone_id).map_err(|e| e.to_string())?;
            Ok(plan_to_json(&plan))
        }
        McpToolKind::ValidatePlan => {
            let zone_id = arg_string(arguments, "zone_id")?;
            let db = open_default().map_err(|e| e.to_string())?;
            let plan = plan_zone(&db, &zone_id).map_err(|e| e.to_string())?;
            let denied = plan
                .actions
                .iter()
                .filter(|a| a.decision.is_denied())
                .count();
            Ok(json!({
                "zone_id": zone_id,
                "actions": plan.actions.len(),
                "denied": denied,
                "valid": denied == 0,
            }))
        }
        McpToolKind::ExplainPlan => {
            let zone_id = arg_string(arguments, "zone_id")?;
            Ok(json!({
                "zone_id": zone_id,
                "message": "Ghost produces deterministic plans from Zone rules. Approve in the desktop UI before execution.",
            }))
        }
        McpToolKind::RequestApproval | McpToolKind::GetApprovalStatus => Err(
            "Approval must be completed in the Ghost desktop UI — MCP clients cannot approve plans"
                .to_string(),
        ),
        McpToolKind::ExecuteApprovedPlan => {
            let token = arg_string(arguments, "approval_token")?;
            let zone_id = arg_string(arguments, "zone_id")?;
            super::execute::execute_approved_plan(&zone_id, &token).map_err(|e| e.to_string())
        }
        McpToolKind::GetRun => {
            let execution_id = arg_string(arguments, "execution_id")?;
            super::execute::get_run_summary(&execution_id)
        }
        McpToolKind::UndoRun => {
            let execution_id = arg_string(arguments, "execution_id")?;
            super::execute::undo_run(&execution_id)
        }
    }
}

pub fn list_tools() -> Vec<Value> {
    McpToolKind::all()
        .iter()
        .map(|kind| {
            json!({
                "name": kind.name(),
                "description": kind.description(),
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "zone_id": { "type": "string" },
                        "approval_token": { "type": "string" },
                        "execution_id": { "type": "string" },
                        "pairing_code": { "type": "string" },
                    },
                },
            })
        })
        .collect()
}

fn tool_kind_from_name(name: &str) -> Result<McpToolKind, String> {
    McpToolKind::all()
        .iter()
        .copied()
        .find(|k| k.name() == name)
        .ok_or_else(|| format!("Unknown MCP tool '{name}'"))
}

fn arg_string(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Missing required argument '{key}'"))
}

fn plan_to_json(plan: &OrganizerPlan) -> Value {
    json!({
        "plan_id": format!("plan_{}", plan.zone_id),
        "status": "awaiting_review",
        "zone_id": plan.zone_id,
        "summary": plan.summary,
        "actions": plan.actions,
        "skipped": plan.skipped,
        "denied_operations": plan.actions.iter().filter(|a| a.decision.is_denied()).count(),
    })
}
