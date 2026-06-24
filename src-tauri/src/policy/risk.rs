//! Risk levels attached to policy decisions.

use serde::{Deserialize, Serialize};

/// How dangerous a capability is once approved. Used to drive confirmation
/// strength and audit emphasis. Distinct from `core::guard::GuardSeverity`
/// (that classifies recorded-workflow findings, not live capability risk).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
