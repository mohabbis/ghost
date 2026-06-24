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
