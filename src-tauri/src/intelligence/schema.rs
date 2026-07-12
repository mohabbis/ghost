use serde::{Deserialize, Serialize};

use super::capability::ProviderId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanningRequest {
    pub objective: String,
    pub zone_id: Option<String>,
    pub file_metadata: Vec<FileMetadataSummary>,
    pub sensitivity: DataSensitivity,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataSensitivity {
    Public,
    InternalMetadata,
    Personal,
    Confidential,
    Secret,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileMetadataSummary {
    pub name: String,
    pub extension: Option<String>,
    pub size_bytes: u64,
    pub created_at: Option<String>,
    pub zone_relative_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanningSuggestion {
    pub objective: String,
    pub classifications: Vec<SuggestedClassification>,
    pub proposed_rules: Vec<SuggestedRule>,
    pub assumptions: Vec<String>,
    pub uncertainty: Vec<UncertainItem>,
    pub confidence: Option<f32>,
    pub provider_id: Option<ProviderId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuggestedClassification {
    pub zone_relative_path: String,
    pub category: String,
    pub confidence: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuggestedRule {
    pub description: String,
    pub pattern_hint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UncertainItem {
    pub zone_relative_path: String,
    pub reason: String,
}

/// Provider output must never contain directly executable filesystem operations.
pub fn suggestion_is_safe(suggestion: &PlanningSuggestion) -> bool {
    let json = serde_json::to_string(suggestion)
        .unwrap_or_default()
        .to_lowercase();
    let forbidden = [
        "\"command\"",
        "\"shell\"",
        "\"execute\"",
        "rm -rf",
        "delete_file",
        "move_file",
        "write_file",
        "filesystem.write",
        "filesystem.read",
    ];
    !forbidden.iter().any(|needle| json.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_suggestion_passes_validation() {
        let suggestion = PlanningSuggestion {
            objective: "Group PDF invoices by year".to_string(),
            classifications: vec![],
            proposed_rules: vec![],
            assumptions: vec![],
            uncertainty: vec![],
            confidence: Some(0.8),
            provider_id: None,
        };
        assert!(suggestion_is_safe(&suggestion));
    }

    #[test]
    fn executable_fields_fail_validation() {
        let suggestion = PlanningSuggestion {
            objective: "bad".to_string(),
            classifications: vec![],
            proposed_rules: vec![SuggestedRule {
                description: "run destructive command".to_string(),
                pattern_hint: Some("rm -rf ~/Downloads".to_string()),
            }],
            assumptions: vec![],
            uncertainty: vec![],
            confidence: None,
            provider_id: None,
        };
        assert!(!suggestion_is_safe(&suggestion));
    }
}
