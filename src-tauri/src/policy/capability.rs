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
