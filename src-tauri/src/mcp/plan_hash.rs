//! Deterministic plan hashing for MCP approval tokens.

use crate::organizer::planner::OrganizerPlan;
use sha2::{Digest, Sha256};

/// Hash the canonical JSON of a plan so approval tokens bind to the exact
/// server-side plan the user reviewed.
pub fn hash_organizer_plan(plan: &OrganizerPlan) -> String {
    let json = serde_json::to_string(plan).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    let hash = hasher.finalize();
    format!(
        "sha256:{}",
        hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
