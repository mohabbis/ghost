use serde::{Deserialize, Serialize};

use crate::core::events::{InputEvent, ReliabilitySettings, Workflow, WorkflowMetadata};

/// Current durable workflow file schema version.
///
/// Versioned workflow files are stored as an object instead of a bare event
/// array so future migrations can be explicit and testable.
pub const CURRENT_WORKFLOW_SCHEMA_VERSION: u32 = 1;

/// Durable workflow file envelope.
///
/// Keep this shape separate from the runtime `Workflow` model. Runtime models can
/// evolve for UI and analysis, while this type defines what Ghost commits to disk.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkflowFile {
    pub schema_version: u32,
    pub name: String,
    pub events: Vec<InputEvent>,
    #[serde(default)]
    pub metadata: WorkflowMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reliability: Option<ReliabilitySettings>,
}

impl WorkflowFile {
    pub fn new(name: &str, events: &[InputEvent]) -> Self {
        WorkflowFile {
            schema_version: CURRENT_WORKFLOW_SCHEMA_VERSION,
            name: name.to_string(),
            events: events.to_vec(),
            metadata: WorkflowMetadata {
                name: name.to_string(),
                ..WorkflowMetadata::default()
            },
            reliability: None,
        }
    }

    pub fn from_workflow(workflow: Workflow) -> Self {
        WorkflowFile {
            schema_version: CURRENT_WORKFLOW_SCHEMA_VERSION,
            name: workflow.name,
            events: workflow.events,
            metadata: workflow.metadata,
            reliability: workflow.reliability,
        }
    }

    pub fn into_workflow(self) -> Workflow {
        Workflow {
            name: self.name,
            events: self.events,
            metadata: self.metadata,
            reliability: self.reliability,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PersistedWorkflow {
    Versioned(WorkflowFile),
    LegacyWorkflow(Workflow),
    LegacyEvents(Vec<InputEvent>),
}

/// Serialize a workflow using the durable versioned file envelope.
pub fn serialize_workflow_file(name: &str, events: &[InputEvent]) -> anyhow::Result<String> {
    let file = WorkflowFile::new(name, events);
    Ok(serde_json::to_string_pretty(&file)?)
}

/// Serialize a complete runtime workflow using the durable versioned file envelope.
pub fn serialize_workflow_with_metadata(workflow: &Workflow) -> anyhow::Result<String> {
    let file = WorkflowFile {
        schema_version: CURRENT_WORKFLOW_SCHEMA_VERSION,
        name: workflow.name.clone(),
        events: workflow.events.clone(),
        metadata: workflow.metadata.clone(),
        reliability: workflow.reliability.clone(),
    };
    Ok(serde_json::to_string_pretty(&file)?)
}

/// Deserialize a workflow file while accepting older on-disk formats.
///
/// Supported inputs:
/// - v1 versioned envelope: `{ "schema_version": 1, "name": ..., "events": ... }`
/// - legacy runtime workflow object: `{ "name": ..., "events": ..., "metadata": ... }`
/// - legacy bare event array: `[ ... ]`
pub fn deserialize_workflow_file(json: &str) -> anyhow::Result<WorkflowFile> {
    let persisted: PersistedWorkflow = serde_json::from_str(json)?;
    match persisted {
        PersistedWorkflow::Versioned(file) => migrate(file),
        PersistedWorkflow::LegacyWorkflow(workflow) => Ok(WorkflowFile::from_workflow(workflow)),
        PersistedWorkflow::LegacyEvents(events) => Ok(WorkflowFile::new("", &events)),
    }
}

/// Deserialize only the event stream for the stable command API.
pub fn deserialize_workflow_events(json: &str) -> anyhow::Result<Vec<InputEvent>> {
    Ok(deserialize_workflow_file(json)?.events)
}

/// Deserialize into the runtime workflow model for metadata-aware commands.
pub fn deserialize_workflow_with_metadata(json: &str) -> anyhow::Result<Workflow> {
    Ok(deserialize_workflow_file(json)?.into_workflow())
}

fn migrate(file: WorkflowFile) -> anyhow::Result<WorkflowFile> {
    match file.schema_version {
        CURRENT_WORKFLOW_SCHEMA_VERSION => Ok(file),
        version => anyhow::bail!(
            "Unsupported workflow schema version {version}; current version is {CURRENT_WORKFLOW_SCHEMA_VERSION}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delay(ms: u64) -> InputEvent {
        InputEvent::Delay {
            ms,
            timestamp: Some(123),
        }
    }

    #[test]
    fn serializes_versioned_workflow_file() {
        let json = serialize_workflow_file("smoke-test", &[delay(250)]).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["schema_version"], CURRENT_WORKFLOW_SCHEMA_VERSION);
        assert_eq!(value["name"], "smoke-test");
        assert_eq!(value["events"].as_array().unwrap().len(), 1);
        assert_eq!(value["metadata"]["name"], "smoke-test");
    }

    #[test]
    fn loads_versioned_workflow_file() {
        let json = serialize_workflow_file("roundtrip", &[delay(500)]).unwrap();
        let file = deserialize_workflow_file(&json).unwrap();

        assert_eq!(file.schema_version, CURRENT_WORKFLOW_SCHEMA_VERSION);
        assert_eq!(file.name, "roundtrip");
        assert_eq!(file.events.len(), 1);
    }

    #[test]
    fn loads_legacy_bare_event_array() {
        let legacy_json = serde_json::to_string(&vec![delay(100)]).unwrap();
        let events = deserialize_workflow_events(&legacy_json).unwrap();

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], InputEvent::Delay { ms: 100, .. }));
    }

    #[test]
    fn loads_legacy_workflow_object() {
        let workflow = Workflow {
            name: "legacy-object".to_string(),
            events: vec![delay(300)],
            metadata: WorkflowMetadata {
                name: "legacy-object".to_string(),
                description: "old runtime workflow".to_string(),
                ..WorkflowMetadata::default()
            },
            reliability: Some(ReliabilitySettings::default()),
        };
        let legacy_json = serde_json::to_string(&workflow).unwrap();

        let file = deserialize_workflow_file(&legacy_json).unwrap();

        assert_eq!(file.schema_version, CURRENT_WORKFLOW_SCHEMA_VERSION);
        assert_eq!(file.name, "legacy-object");
        assert_eq!(file.metadata.description, "old runtime workflow");
        assert!(file.reliability.is_some());
    }

    #[test]
    fn rejects_unknown_future_schema_version() {
        let json = r#"
        {
            "schema_version": 999,
            "name": "future",
            "events": [],
            "metadata": {
                "name": "future",
                "description": "",
                "tags": [],
                "created_at": 0,
                "updated_at": 0,
                "estimated_duration_ms": 0,
                "reliability_score": 0.0,
                "element_confidence": 0.0
            }
        }
        "#;

        let error = deserialize_workflow_file(json).unwrap_err().to_string();
        assert!(error.contains("Unsupported workflow schema version 999"));
    }
}
