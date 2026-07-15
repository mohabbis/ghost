use super::schema::{FileMetadataSummary, PlanningRequest};

/// Shared redaction for provider requests and integration exports.
pub fn redact_file_metadata(items: &[FileMetadataSummary]) -> Vec<FileMetadataSummary> {
    items
        .iter()
        .map(|item| FileMetadataSummary {
            name: item.name.clone(),
            extension: item.extension.clone(),
            size_bytes: item.size_bytes,
            created_at: item.created_at.clone(),
            zone_relative_path: item.zone_relative_path.clone(),
        })
        .collect()
}

pub fn redact_planning_request(mut request: PlanningRequest) -> PlanningRequest {
    request.file_metadata = redact_file_metadata(&request.file_metadata);
    request
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intelligence::schema::DataSensitivity;

    #[test]
    fn redaction_preserves_zone_relative_paths_only() {
        let request = PlanningRequest {
            objective: "organize".to_string(),
            zone_id: Some("zone-1".to_string()),
            file_metadata: vec![FileMetadataSummary {
                name: "invoice.pdf".to_string(),
                extension: Some("pdf".to_string()),
                size_bytes: 100,
                created_at: Some("2026-07-08".to_string()),
                zone_relative_path: "invoice.pdf".to_string(),
            }],
            sensitivity: DataSensitivity::InternalMetadata,
        };
        let redacted = redact_planning_request(request);
        assert_eq!(redacted.file_metadata[0].name, "invoice.pdf");
        assert!(
            !serde_json::to_string(&redacted)
                .unwrap()
                .contains("/Users/")
        );
    }
}
