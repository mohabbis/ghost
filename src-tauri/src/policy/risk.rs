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

#[cfg(test)]
mod tests {
    use super::*;

    /// The serialized tokens cross the IPC boundary into the frontend. A rename
    /// here would silently break the confirmation UI, so pin them down.
    #[test]
    fn serializes_as_lowercase_tokens() {
        assert_eq!(serde_json::to_string(&RiskLevel::Low).unwrap(), "\"low\"");
        assert_eq!(
            serde_json::to_string(&RiskLevel::Medium).unwrap(),
            "\"medium\""
        );
        assert_eq!(serde_json::to_string(&RiskLevel::High).unwrap(), "\"high\"");
        assert_eq!(
            serde_json::to_string(&RiskLevel::Critical).unwrap(),
            "\"critical\""
        );
    }

    #[test]
    fn round_trips_through_json() {
        for level in [
            RiskLevel::Low,
            RiskLevel::Medium,
            RiskLevel::High,
            RiskLevel::Critical,
        ] {
            let json = serde_json::to_string(&level).unwrap();
            let back: RiskLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(level, back);
        }
    }
}
