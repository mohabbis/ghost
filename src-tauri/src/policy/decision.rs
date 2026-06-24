//! The outcome of evaluating a capability against the active policy.

use super::risk::RiskLevel;
use serde::{Deserialize, Serialize};

/// The result of a policy evaluation. The deterministic core executes only
/// `Allow`; `RequireConfirmation` must surface to the user for approval; `Deny`
/// stops the action entirely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "decision")]
pub enum PolicyDecision {
    /// The action is permitted without further prompting.
    Allow,
    /// The action is refused; `reason` explains why.
    Deny { reason: String },
    /// The action is permitted only after explicit user confirmation.
    RequireConfirmation { reason: String, risk: RiskLevel },
}

impl PolicyDecision {
    /// Whether the action may proceed with no further prompting.
    pub fn is_allowed(&self) -> bool {
        matches!(self, PolicyDecision::Allow)
    }

    /// Whether the action is refused outright.
    pub fn is_denied(&self) -> bool {
        matches!(self, PolicyDecision::Deny { .. })
    }

    /// Whether the action needs explicit user confirmation first.
    pub fn needs_confirmation(&self) -> bool {
        matches!(self, PolicyDecision::RequireConfirmation { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicates_are_mutually_exclusive() {
        let allow = PolicyDecision::Allow;
        assert!(allow.is_allowed());
        assert!(!allow.is_denied());
        assert!(!allow.needs_confirmation());

        let deny = PolicyDecision::Deny {
            reason: "out of bounds".into(),
        };
        assert!(!deny.is_allowed());
        assert!(deny.is_denied());
        assert!(!deny.needs_confirmation());

        let confirm = PolicyDecision::RequireConfirmation {
            reason: "in-boundary move".into(),
            risk: RiskLevel::Medium,
        };
        assert!(!confirm.is_allowed());
        assert!(!confirm.is_denied());
        assert!(confirm.needs_confirmation());
    }

    /// The internally-tagged `decision` field and snake_case variant names are a
    /// frontend contract; pin the wire shape so a refactor can't quietly drift.
    #[test]
    fn serializes_with_internal_decision_tag() {
        assert_eq!(
            serde_json::to_value(PolicyDecision::Allow).unwrap(),
            serde_json::json!({ "decision": "allow" })
        );
        assert_eq!(
            serde_json::to_value(PolicyDecision::Deny {
                reason: "nope".into()
            })
            .unwrap(),
            serde_json::json!({ "decision": "deny", "reason": "nope" })
        );
        assert_eq!(
            serde_json::to_value(PolicyDecision::RequireConfirmation {
                reason: "confirm?".into(),
                risk: RiskLevel::High,
            })
            .unwrap(),
            serde_json::json!({
                "decision": "require_confirmation",
                "reason": "confirm?",
                "risk": "high"
            })
        );
    }

    #[test]
    fn round_trips_through_json() {
        let cases = [
            PolicyDecision::Allow,
            PolicyDecision::Deny {
                reason: "denied".into(),
            },
            PolicyDecision::RequireConfirmation {
                reason: "confirm".into(),
                risk: RiskLevel::Critical,
            },
        ];
        for decision in cases {
            let json = serde_json::to_string(&decision).unwrap();
            let back: PolicyDecision = serde_json::from_str(&json).unwrap();
            assert_eq!(decision, back);
        }
    }
}
