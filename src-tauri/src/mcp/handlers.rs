//! Headless MCP tool handlers — domain logic only, no Tauri.

use crate::mcp::pending::ApprovalRequestStatus;
use crate::mcp::tools::McpToolKind;
use crate::mcp::{hash_organizer_plan, pending};
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
        McpToolKind::RequestApproval => {
            let zone_id = arg_string(arguments, "zone_id")?;
            let db = open_default().map_err(|e| e.to_string())?;
            let plan = plan_zone(&db, &zone_id).map_err(|e| e.to_string())?;
            let plan_hash = hash_organizer_plan(&plan);
            let req = pending::create_request(&zone_id, &plan_hash);
            Ok(json!({
                "request_id": req.request_id,
                "zone_id": zone_id,
                "plan_id": req.plan_id,
                "status": "pending",
                "expires_at": req.expires_at.to_rfc3339(),
                "message": "Open Ghost Organizer, review the plan for this Zone, and issue an MCP approval token.",
            }))
        }
        McpToolKind::GetApprovalStatus => {
            let request_id = arg_string(arguments, "request_id")?;
            let req = pending::get_request(&request_id)
                .ok_or_else(|| format!("Unknown approval request '{request_id}'"))?;
            Ok(json!({
                "request_id": req.request_id,
                "zone_id": req.zone_id,
                "plan_id": req.plan_id,
                "status": approval_status_label(&req.status),
                "expires_at": req.expires_at.to_rfc3339(),
                "approved_at": req.approved_at.map(|t| t.to_rfc3339()),
            }))
        }
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
                        "request_id": { "type": "string" },
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

fn approval_status_label(status: &ApprovalRequestStatus) -> &'static str {
    match status {
        ApprovalRequestStatus::Pending => "pending",
        ApprovalRequestStatus::Approved => "approved",
        ApprovalRequestStatus::Denied => "denied",
        ApprovalRequestStatus::Expired => "expired",
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_tool_name_is_rejected() {
        let err = handle_tool("ghost.delete_everything", &json!({})).unwrap_err();
        assert!(err.contains("Unknown MCP tool"), "got: {err}");
    }

    #[test]
    fn status_tool_reports_the_trust_pipeline_without_touching_disk() {
        // `ghost.status` is a pure metadata handler — it must never require a
        // Zone, a plan, or the database to answer.
        let out = handle_tool(McpToolKind::Status.name(), &json!({})).unwrap();
        assert_eq!(out["app"], "ghost");
        assert!(out["trust_pipeline"].as_str().unwrap().contains("Approval"));
    }

    #[test]
    fn explain_plan_echoes_the_requested_zone() {
        let out = handle_tool(
            McpToolKind::ExplainPlan.name(),
            &json!({ "zone_id": "zone-42" }),
        )
        .unwrap();
        assert_eq!(out["zone_id"], "zone-42");
        assert!(out["message"].as_str().unwrap().contains("Approve"));
    }

    #[test]
    fn zone_scoped_tools_require_a_zone_id_argument() {
        // A tool that operates on a Zone must fail loudly on a missing
        // argument rather than silently defaulting to some Zone.
        let err = handle_tool(McpToolKind::ExplainPlan.name(), &json!({})).unwrap_err();
        assert!(err.contains("Missing required argument"), "got: {err}");
        assert!(err.contains("zone_id"), "got: {err}");
    }

    #[test]
    fn get_approval_status_rejects_unknown_request_ids() {
        let err = handle_tool(
            McpToolKind::GetApprovalStatus.name(),
            &json!({ "request_id": "does-not-exist" }),
        )
        .unwrap_err();
        assert!(err.contains("Unknown approval request"), "got: {err}");
    }

    #[test]
    fn list_tools_advertises_every_tool_kind_once() {
        let tools = list_tools();
        assert_eq!(tools.len(), McpToolKind::all().len());
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for kind in McpToolKind::all() {
            assert!(
                names.contains(&kind.name()),
                "list_tools missing {}",
                kind.name()
            );
        }
        // The mutating execute tool must be advertised with an input schema so
        // clients know it takes an approval token, not a bare Zone id.
        let execute = tools
            .iter()
            .find(|t| t["name"] == McpToolKind::ExecuteApprovedPlan.name())
            .expect("execute tool advertised");
        assert!(execute["inputSchema"]["properties"]
            .get("approval_token")
            .is_some());
    }
}
