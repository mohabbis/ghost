use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditActor {
    User,
    System,
    McpClient { client_id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntegrationAuditEvent {
    pub event_id: String,
    pub event_type: String,
    pub actor: AuditActor,
    pub provider: Option<String>,
    pub integration: Option<String>,
    pub resource_scope: Option<String>,
    pub data_categories: Vec<String>,
    pub result: String,
    pub created_at: DateTime<Utc>,
}

impl IntegrationAuditEvent {
    pub fn new(
        event_type: impl Into<String>,
        actor: AuditActor,
        result: impl Into<String>,
    ) -> Self {
        IntegrationAuditEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type: event_type.into(),
            actor,
            provider: None,
            integration: None,
            resource_scope: None,
            data_categories: vec![],
            result: result.into(),
            created_at: Utc::now(),
        }
    }
}
