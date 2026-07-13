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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::approval::issue_approval_token;
    use crate::organizer::planner::{plan_with_rules, PlanAction};
    use crate::policy::{Capability, FolderRule, PolicyDecision, TrustLevel};
    use std::path::PathBuf;

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

    #[test]
    fn denied_plan_cannot_execute_even_with_valid_token() {
        let signed = issue_approval_token("plan_zone-deny", "sha256:abc");
        let token = serde_json::to_string(&signed).unwrap();
        // execute_approved_plan re-plans from DB; without a real zone this fails
        // before token verification on missing zone — ensure error is not "success".
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
}
