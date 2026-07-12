//! System prompts for suggestion-only planning (never executable operations).

use super::schema::{DataSensitivity, PlanningRequest};

pub fn planning_system_prompt() -> &'static str {
    "You are a file-organization planning assistant for Ghost. \
     Respond with ONLY valid JSON matching this schema (no markdown, no prose): \
     {\"objective\": string, \"classifications\": [{\"zone_relative_path\": string, \"category\": string, \"confidence\": number|null}], \
     \"proposed_rules\": [{\"description\": string, \"pattern_hint\": string|null}], \
     \"assumptions\": [string], \"uncertainty\": [{\"zone_relative_path\": string, \"reason\": string}], \
     \"confidence\": number|null}. \
     Do NOT include shell commands, file paths outside the zone, delete/move/write operations, \
     or any executable instructions. Suggest categories and rules only; Ghost converts approved suggestions into deterministic actions."
}

pub fn planning_user_prompt(request: &PlanningRequest) -> String {
    let sensitivity = match request.sensitivity {
        DataSensitivity::Public => "public",
        DataSensitivity::InternalMetadata => "internal_metadata",
        DataSensitivity::Personal => "personal",
        DataSensitivity::Confidential => "confidential",
        DataSensitivity::Secret => "secret",
    };
    serde_json::json!({
        "objective": request.objective,
        "zone_id": request.zone_id,
        "sensitivity": sensitivity,
        "files": request.file_metadata,
    })
    .to_string()
}
