//! Deterministic plan hashing for MCP approval tokens.

use crate::organizer::planner::OrganizerPlan;
use crate::policy::{evaluate_compressed, fingerprint_events};
use sha2::{Digest, Sha256};

/// Hash the canonical JSON of a plan so approval tokens bind to the exact
/// server-side plan the user reviewed.
pub fn hash_organizer_plan(plan: &OrganizerPlan) -> String {
    let json = serde_json::to_string(plan).unwrap_or_default();
    hash_bytes(json.as_bytes())
}

/// Stable identifier for the saved routine an approval covers.
pub fn routine_plan_id(workflow_name: &str) -> String {
    format!("routine:{workflow_name}")
}

/// Hash the exact saved event stream and the policy decisions Ghost will
/// enforce. The review response never includes event contents, but changing
/// even one recorded key/click invalidates the approval.
pub fn hash_routine_plan(
    workflow_name: &str,
    events: &[crate::core::events::InputEvent],
) -> String {
    let report = crate::core::compression::compress(events);
    let policy = evaluate_compressed(&report);
    let canonical = serde_json::to_vec(&(
        routine_plan_id(workflow_name),
        fingerprint_events(events),
        policy,
    ))
    .unwrap_or_default();
    hash_bytes(&canonical)
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hash = hasher.finalize();
    format!(
        "sha256:{}",
        hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::InputEvent;
    use crate::organizer::planner::PlanSummary;

    #[test]
    fn same_plan_yields_same_hash() {
        let plan = OrganizerPlan {
            zone_id: "z1".to_string(),
            actions: vec![],
            skipped: vec![],
            summary: PlanSummary::default(),
        };
        assert_eq!(hash_organizer_plan(&plan), hash_organizer_plan(&plan));
    }

    #[test]
    fn routine_hash_changes_with_events() {
        let first = vec![InputEvent::Delay {
            ms: 300,
            timestamp: None,
        }];
        let changed = vec![InputEvent::Delay {
            ms: 301,
            timestamp: None,
        }];
        assert_eq!(
            hash_routine_plan("transfer", &first),
            hash_routine_plan("transfer", &first)
        );
        assert_ne!(
            hash_routine_plan("transfer", &first),
            hash_routine_plan("transfer", &changed)
        );
    }
}
