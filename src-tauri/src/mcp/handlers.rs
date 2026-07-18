//! Headless MCP tool handlers — domain logic only, no Tauri.

use crate::core::compression::{CompressedStep, compress};
use crate::core::dry_run::preview_workflow;
use crate::engine::GhostEngine;
use crate::mcp::pending;
use crate::mcp::pending::ApprovalRequestStatus;
use crate::mcp::plan_hash::{hash_organizer_plan, hash_routine_plan, routine_plan_id};
use crate::mcp::tools::McpToolKind;
use crate::organizer::planner::{OrganizerPlan, plan_zone};
use crate::policy::evaluate_compressed;
use crate::storage::open_default;
use crate::storage::zones::list_zones;
use serde_json::{Value, json};

pub fn handle_tool(name: &str, arguments: &Value) -> Result<Value, String> {
    let kind = tool_kind_from_name(name)?;
    match kind {
        McpToolKind::Status => Ok(json!({
            "app": "ghost",
            "mcp": "local-stdio",
            "clients": {
                "supported_now": ["claude_desktop", "cursor"],
                "transport": "stdio",
                "chatgpt": "not_supported_on_stock_stdio — needs remote HTTP/connector (experimental)"
            },
            "trust_pipeline": "Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo",
            "routines": "list/preview/request_approval/execute_approved via ghost.*_routine tools",
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
                "kind": "organizer",
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
                "kind": match req.kind {
                    pending::ApprovalRequestKind::Organizer => "organizer",
                    pending::ApprovalRequestKind::Routine => "routine",
                },
                "zone_id": req.zone_id,
                "routine_name": req.routine_name,
                "plan_id": req.plan_id,
                "plan_hash": req.plan_hash,
                "status": approval_status_label(&req.status),
                "expires_at": req.expires_at.to_rfc3339(),
                "approved_at": req.approved_at.map(|t| t.to_rfc3339()),
                // Token is only present after local Ghost approval. Clients
                // that know the unguessable request_id can poll for it.
                "approval_token": req.approval_token,
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
        McpToolKind::ListRoutines => list_routines(),
        McpToolKind::PreviewRoutine => {
            let routine_name = arg_string(arguments, "routine_name")?;
            preview_routine(&routine_name)
        }
        McpToolKind::RequestRoutineApproval => {
            let routine_name = arg_string(arguments, "routine_name")?;
            request_routine_approval(&routine_name)
        }
        McpToolKind::ExecuteApprovedRoutine => {
            let token = arg_string(arguments, "approval_token")?;
            let routine_name = arg_string(arguments, "routine_name")?;
            let engine = GhostEngine::new();
            super::execute::execute_approved_routine(&routine_name, &token, &engine)
        }
    }
}

fn list_routines() -> Result<Value, String> {
    let names = GhostEngine::list_workflows().map_err(|e| e.to_string())?;
    Ok(json!({
        "routines": names.into_iter().map(|name| json!({
            "name": name,
            // Metadata only — never event counts that could leak size of
            // sensitive capture sessions without an unlock.
        })).collect::<Vec<_>>(),
        "note": "Names only. Use ghost.preview_routine for redacted steps.",
    }))
}

fn load_routine_events(
    routine_name: &str,
) -> Result<(GhostEngine, Vec<crate::core::events::InputEvent>), String> {
    let engine = GhostEngine::new();
    if !engine.auth().is_unlocked() {
        return Err(
            "Ghost's vault is locked. Unlock Ghost before previewing this routine.".to_string(),
        );
    }
    let events = engine
        .load_workflow(routine_name)
        .map_err(|e| format!("Could not load saved routine '{routine_name}': {e}"))?;
    Ok((engine, events))
}

fn preview_routine(routine_name: &str) -> Result<Value, String> {
    let (_engine, events) = load_routine_events(routine_name)?;
    let report = compress(&events);
    let policy = evaluate_compressed(&report);
    let plan_hash = hash_routine_plan(routine_name, &events);
    let dry_run = preview_workflow(&events);
    let steps = redacted_steps_json(&report.steps);

    Ok(json!({
        "routine_name": routine_name,
        "plan_id": routine_plan_id(routine_name),
        "plan_hash": plan_hash,
        "status": "awaiting_review",
        "event_count": events.len(),
        "compressed_step_count": report.compressed_step_count,
        "steps": steps,
        "dry_run": dry_run,
        "policy": redacted_policy_json(&policy),
        "warnings": report.warnings,
        "message": "Review these steps in Ghost, then approve locally. Typed text is never included.",
    }))
}

fn redact_key_description(desc: &str) -> String {
    // Unclassified single keystrokes compress to Unknown with the raw
    // character in the description — never surface that over MCP.
    if desc.starts_with("Press key ") || desc.starts_with("Release key ") {
        if desc.starts_with("Release") {
            "Release key (redacted)".into()
        } else {
            "Press key (redacted)".into()
        }
    } else {
        desc.to_string()
    }
}

fn redacted_steps_json(steps: &[CompressedStep]) -> Vec<Value> {
    steps
        .iter()
        .enumerate()
        .map(|(index, step)| match step {
            CompressedStep::TypeText(t) => json!({
                "index": index,
                "kind": "type_text",
                "char_count": t.char_count,
                "redacted": true,
                "secure_field": t.secure_field,
                "target": t.target,
                "text": Value::Null,
                "confidence": t.confidence,
            }),
            CompressedStep::Unknown(u) => json!({
                "index": index,
                "kind": "unknown",
                "description": redact_key_description(&u.description),
                "raw_event_count": u.raw_event_count,
            }),
            other => {
                let mut value = serde_json::to_value(other).unwrap_or(json!({}));
                if let Some(obj) = value.as_object_mut() {
                    obj.insert("index".into(), json!(index));
                    if let Some(inner) = obj.get_mut("text") {
                        *inner = Value::Null;
                    }
                }
                value
            }
        })
        .collect()
}

fn redacted_policy_json(policy: &crate::policy::RoutinePolicyPlan) -> Value {
    let steps: Vec<Value> = policy
        .steps
        .iter()
        .map(|s| {
            let mut capability = serde_json::to_value(&s.capability).unwrap_or(json!({}));
            if let Some(desc) = capability
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
            {
                capability["description"] = json!(redact_key_description(&desc));
            }
            // Also redact deny reasons that echo the unknown description.
            let mut decision = serde_json::to_value(&s.decision).unwrap_or(json!({}));
            if let Some(reason) = decision.get("reason").and_then(|v| v.as_str())
                && (reason.contains("Press key '") || reason.contains("Release key '"))
            {
                decision["reason"] =
                    json!("Unrecognized routine step cannot be approved: keystroke (redacted)");
            }
            json!({
                "step_index": s.step_index,
                "capability": capability,
                "decision": decision,
            })
        })
        .collect();
    json!({
        "can_proceed_with_approvals": policy.can_proceed_with_approvals,
        "denied_count": policy.denied_count,
        "confirmation_count": policy.confirmation_count,
        "allow_count": policy.allow_count,
        "steps": steps,
    })
}

fn request_routine_approval(routine_name: &str) -> Result<Value, String> {
    let (_engine, events) = load_routine_events(routine_name)?;
    let report = compress(&events);
    let policy = evaluate_compressed(&report);
    crate::policy::ensure_replayable(&policy)?;
    let plan_id = routine_plan_id(routine_name);
    let plan_hash = hash_routine_plan(routine_name, &events);
    let req = pending::create_routine_request(routine_name, &plan_id, &plan_hash);
    Ok(json!({
        "request_id": req.request_id,
        "kind": "routine",
        "routine_name": routine_name,
        "plan_id": plan_id,
        "plan_hash": plan_hash,
        "status": "pending",
        "expires_at": req.expires_at.to_rfc3339(),
        "policy": {
            "can_proceed_with_approvals": policy.can_proceed_with_approvals,
            "denied_count": policy.denied_count,
            "confirmation_count": policy.confirmation_count,
        },
        "message": "Open Ghost, review the routine steps, and approve the MCP request. Poll ghost.get_approval_status until approved, then call ghost.execute_approved_routine.",
    }))
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
                        "routine_name": { "type": "string" },
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
    use crate::core::events::{InputEvent, KeyAction};

    #[test]
    fn unknown_tool_name_is_rejected() {
        let err = handle_tool("ghost.delete_everything", &json!({})).unwrap_err();
        assert!(err.contains("Unknown MCP tool"), "got: {err}");
    }

    #[test]
    fn status_tool_reports_the_trust_pipeline_without_touching_disk() {
        let out = handle_tool(McpToolKind::Status.name(), &json!({})).unwrap();
        assert_eq!(out["app"], "ghost");
        assert!(out["trust_pipeline"].as_str().unwrap().contains("Approval"));
        assert!(
            out["clients"]["chatgpt"]
                .as_str()
                .unwrap()
                .contains("not_supported")
        );
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
        let execute = tools
            .iter()
            .find(|t| t["name"] == McpToolKind::ExecuteApprovedRoutine.name())
            .expect("execute routine tool advertised");
        assert!(
            execute["inputSchema"]["properties"]
                .get("approval_token")
                .is_some()
        );
        assert!(
            execute["inputSchema"]["properties"]
                .get("routine_name")
                .is_some()
        );
    }

    #[test]
    fn preview_redacts_typed_text() {
        let engine = GhostEngine::new();
        if engine.auth().is_configured() {
            return;
        }
        let name = format!("mcp-preview-{}", uuid::Uuid::new_v4());
        let secret = "hunter2-super-secret";
        let events = vec![
            InputEvent::Delay {
                ms: 300,
                timestamp: None,
            },
            InputEvent::Key {
                code: 4,
                chars: secret.to_string(),
                modifiers: 0,
                action: KeyAction::Down,
                timestamp: None,
                retry_count: None,
                semantic_tag: None,
            },
            InputEvent::Key {
                code: 4,
                chars: secret.to_string(),
                modifiers: 0,
                action: KeyAction::Up,
                timestamp: None,
                retry_count: None,
                semantic_tag: None,
            },
        ];
        let path = engine.save_workflow(&name, &events).unwrap();
        let out = preview_routine(&name).unwrap();
        let serialized = serde_json::to_string(&out).unwrap();
        assert!(!serialized.contains(secret), "preview leaked typed text");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn preview_loads_ui_saved_workflow_object() {
        // UI saves `{ name, events, metadata, ... }` via save_workflow_with_metadata.
        // MCP preview must accept that shape (not only bare event arrays).
        let engine = GhostEngine::new();
        if engine.auth().is_configured() {
            return;
        }
        let name = format!("mcp-object-{}", uuid::Uuid::new_v4());
        let workflow = crate::core::events::Workflow {
            name: name.clone(),
            events: vec![InputEvent::Delay {
                ms: 250,
                timestamp: None,
            }],
            metadata: Default::default(),
            reliability: None,
        };
        let path = engine.save_workflow_with_metadata(&workflow).unwrap();
        let out = preview_routine(&name).expect("preview should load Workflow object");
        assert_eq!(out["routine_name"], name);
        assert_eq!(out["event_count"], 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn execute_approved_routine_without_token_is_denied() {
        let err = handle_tool(
            McpToolKind::ExecuteApprovedRoutine.name(),
            &json!({ "routine_name": "missing", "approval_token": "bad" }),
        )
        .unwrap_err();
        assert!(!err.is_empty());
    }
}
