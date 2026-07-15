//! Parse structured JSON suggestions from provider text responses.

use super::errors::ProviderError;
use super::schema::{PlanningSuggestion, suggestion_is_safe};

/// Extract and validate a `PlanningSuggestion` from raw model text.
pub fn parse_planning_suggestion(content: &str) -> Result<PlanningSuggestion, ProviderError> {
    let json_str = extract_json_object(content);
    let suggestion: PlanningSuggestion =
        serde_json::from_str(&json_str).map_err(|_| ProviderError::InvalidResponse)?;
    if !suggestion_is_safe(&suggestion) {
        return Err(ProviderError::SchemaRejected);
    }
    Ok(suggestion)
}

fn extract_json_object(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return trimmed.to_string();
    }
    if let Some(start) = trimmed.find("```json") {
        let rest = &trimmed[start + 7..];
        if let Some(end) = rest.find("```") {
            return rest[..end].trim().to_string();
        }
    }
    if let Some(start) = trimmed.find("```") {
        let rest = &trimmed[start + 3..];
        if let Some(newline) = rest.find('\n') {
            let body = &rest[newline + 1..];
            if let Some(end) = body.find("```") {
                return body[..end].trim().to_string();
            }
        }
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}'))
        && start < end
    {
        return trimmed[start..=end].to_string();
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_json() {
        let raw = r#"Here is the plan:
```json
{"objective":"Organize PDFs","classifications":[],"proposed_rules":[],"assumptions":[],"uncertainty":[],"confidence":0.9}
```"#;
        let s = parse_planning_suggestion(raw).unwrap();
        assert_eq!(s.objective, "Organize PDFs");
    }

    #[test]
    fn rejects_executable_output() {
        let raw = r#"{"objective":"x","classifications":[],"proposed_rules":[{"description":"bad","pattern_hint":"rm -rf"}],"assumptions":[],"uncertainty":[]}"#;
        assert!(matches!(
            parse_planning_suggestion(raw),
            Err(ProviderError::SchemaRejected)
        ));
    }
}
