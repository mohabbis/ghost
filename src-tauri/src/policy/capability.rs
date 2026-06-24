//! Capabilities: the concrete actions Ghost may attempt.
//!
//! Every meaningful operation must be expressed as a `Capability` and pass
//! through the policy engine before execution:
//!
//! ```text
//! Intent -> Plan -> Policy -> Approval -> Execution -> Audit -> Undo
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single concrete action a plan may request. The policy engine maps each of
/// these to a [`crate::policy::PolicyDecision`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Capability {
    /// Read the contents of a folder.
    ReadFolder { path: PathBuf },
    /// Create a new folder.
    CreateFolder { path: PathBuf },
    /// Rename a file (usually within the same folder).
    RenameFile { from: PathBuf, to: PathBuf },
    /// Move a file from one location to another.
    MoveFile { from: PathBuf, to: PathBuf },
    /// Copy a file from one location to another.
    CopyFile { from: PathBuf, to: PathBuf },
    /// Delete a file. Denied in the Organizer MVP.
    DeleteFile { path: PathBuf },
    /// Begin OS input recording. Out of Organizer scope.
    StartRecording,
    /// Replay a recorded workflow. Out of Organizer scope.
    ReplayWorkflow { workflow_id: String },
    /// Capture the screen. Out of Organizer scope.
    CaptureScreen,
    /// Reach a network host. Denied in the Organizer MVP.
    UseNetwork { host: String },
    /// Ask an LLM to generate a workflow. Suggestion-only; never a direct action.
    GenerateWorkflowFromPrompt,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `kind` tag and snake_case variant names are the contract the planner,
    /// the frontend, and any audit record all read. Pin the wire shape for a
    /// representative spread: struct variants (with paths) and a unit variant.
    #[test]
    fn serializes_with_kind_tag() {
        assert_eq!(
            serde_json::to_value(Capability::ReadFolder {
                path: PathBuf::from("/home/u/Downloads"),
            })
            .unwrap(),
            serde_json::json!({ "kind": "read_folder", "path": "/home/u/Downloads" })
        );
        assert_eq!(
            serde_json::to_value(Capability::MoveFile {
                from: PathBuf::from("/a"),
                to: PathBuf::from("/b"),
            })
            .unwrap(),
            serde_json::json!({ "kind": "move_file", "from": "/a", "to": "/b" })
        );
        assert_eq!(
            serde_json::to_value(Capability::StartRecording).unwrap(),
            serde_json::json!({ "kind": "start_recording" })
        );
        assert_eq!(
            serde_json::to_value(Capability::GenerateWorkflowFromPrompt).unwrap(),
            serde_json::json!({ "kind": "generate_workflow_from_prompt" })
        );
    }

    #[test]
    fn round_trips_every_variant_through_json() {
        let cases = [
            Capability::ReadFolder {
                path: PathBuf::from("/p"),
            },
            Capability::CreateFolder {
                path: PathBuf::from("/p"),
            },
            Capability::RenameFile {
                from: PathBuf::from("/a"),
                to: PathBuf::from("/b"),
            },
            Capability::MoveFile {
                from: PathBuf::from("/a"),
                to: PathBuf::from("/b"),
            },
            Capability::CopyFile {
                from: PathBuf::from("/a"),
                to: PathBuf::from("/b"),
            },
            Capability::DeleteFile {
                path: PathBuf::from("/p"),
            },
            Capability::StartRecording,
            Capability::ReplayWorkflow {
                workflow_id: "wf-1".into(),
            },
            Capability::CaptureScreen,
            Capability::UseNetwork {
                host: "example.com".into(),
            },
            Capability::GenerateWorkflowFromPrompt,
        ];
        for cap in cases {
            let json = serde_json::to_string(&cap).unwrap();
            let back: Capability = serde_json::from_str(&json).unwrap();
            assert_eq!(cap, back);
        }
    }
}
