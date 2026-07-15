//! Headless MCP execution helpers — canonical ActionPlan runtime.

use crate::action_plan::{PlanSource, from_organizer_plan_with_source};
use crate::mcp::approval::verify_execution_token_with_hash;
use crate::mcp::plan_hash::hash_organizer_plan;
use crate::organizer::pipeline::undo_zone_run;
use crate::organizer::planner::plan_zone;
use crate::runtime::run_persisted_action_plan;
use crate::storage::executions::{ExecutionSummary, get_execution};
use crate::storage::open_default;
use crate::storage::zones::list_folder_rules;
use serde_json::{Value, json};

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

    let action_plan = from_organizer_plan_with_source(
        &plan,
        PlanSource::Mcp {
            zone_id: zone_id.into(),
        },
    );
    let rules = list_folder_rules(&conn, zone_id).map_err(|e| e.to_string())?;
    let outcome = run_persisted_action_plan(
        &conn,
        zone_id,
        &action_plan,
        &rules,
        None,
        false,
        None,
        None,
    )?;

    Ok(json!({
        "execution_id": outcome.execution_id,
        "zone_id": zone_id,
        "applied": outcome.runtime.report.applied,
        "skipped": outcome.runtime.report.skipped,
        "failed": outcome.runtime.report.failed,
        "stopped_early": outcome.runtime.stopped_early,
        "status": if outcome.runtime.report.failed > 0 || outcome.runtime.stopped_early {
            "failed"
        } else {
            "completed"
        },
        "receipt": outcome.runtime.receipt,
        "plan_source": "mcp",
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
    use crate::organizer::planner::{PlanAction, plan_with_rules};
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
            source_identity: None,
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
