//! The audit log: an append-only record of what the executor did.
//!
//! Every action the executor touches — applied, skipped, or failed — produces
//! one [`AuditEvent`]. The log is a pure, serializable value (no IO); the
//! executor builds it and the UI/persistence layers read it. This is the
//! "Audit" step of the trust pipeline made concrete and inspectable.

use crate::policy::Capability;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// What happened when the executor handled one capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum ActionOutcome {
    /// The operation ran and was verified.
    Applied,
    /// The operation was intentionally not run; `reason` explains why (policy
    /// denial, missing source, target already occupied, …).
    Skipped { reason: String },
    /// The operation was attempted but errored; `error` carries the detail.
    Failed { error: String },
}

/// One row in the audit log: the capability, its outcome, and when it happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    /// The action that was considered, expressed as a policy capability.
    pub capability: Capability,
    /// The result of considering it.
    pub outcome: ActionOutcome,
    /// Epoch-seconds timestamp, as a string (matches the storage convention and
    /// avoids pulling in a date crate). Best-effort; never panics on clock skew.
    pub at: String,
}

/// An append-only, serializable ledger of [`AuditEvent`]s. Serializes
/// transparently as a JSON array so the wire shape is just the events.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AuditLog {
    events: Vec<AuditEvent>,
}

impl AuditLog {
    /// A new, empty log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one event, stamped with the current time.
    pub fn record(&mut self, capability: Capability, outcome: ActionOutcome) {
        self.events.push(AuditEvent {
            capability,
            outcome,
            at: now_ts(),
        });
    }

    /// The recorded events, in the order they happened.
    pub fn events(&self) -> &[AuditEvent] {
        &self.events
    }

    /// How many events have been recorded.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether nothing has been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Epoch-seconds timestamp as a string. Never panics on clock skew.
fn now_ts() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn records_events_in_order() {
        let mut log = AuditLog::new();
        assert!(log.is_empty());
        log.record(
            Capability::CreateFolder {
                path: PathBuf::from("/z/Documents"),
            },
            ActionOutcome::Applied,
        );
        log.record(
            Capability::MoveFile {
                from: PathBuf::from("/z/a.pdf"),
                to: PathBuf::from("/z/Documents/a.pdf"),
            },
            ActionOutcome::Skipped {
                reason: "target already exists".into(),
            },
        );
        assert_eq!(log.len(), 2);
        assert_eq!(log.events()[0].outcome, ActionOutcome::Applied);
        assert!(matches!(
            log.events()[1].outcome,
            ActionOutcome::Skipped { .. }
        ));
    }

    /// The `outcome` tag and snake_case names are a frontend/persistence
    /// contract; pin the wire shape so a refactor can't quietly drift it.
    #[test]
    fn outcome_serializes_with_tag() {
        assert_eq!(
            serde_json::to_value(ActionOutcome::Applied).unwrap(),
            serde_json::json!({ "outcome": "applied" })
        );
        assert_eq!(
            serde_json::to_value(ActionOutcome::Failed {
                error: "boom".into()
            })
            .unwrap(),
            serde_json::json!({ "outcome": "failed", "error": "boom" })
        );
    }

    /// The log serializes transparently as a bare array of events.
    #[test]
    fn log_serializes_transparently_as_array() {
        let mut log = AuditLog::new();
        log.record(
            Capability::CreateFolder {
                path: PathBuf::from("/z/Documents"),
            },
            ActionOutcome::Applied,
        );
        let value = serde_json::to_value(&log).unwrap();
        assert!(value.is_array());
        assert_eq!(value.as_array().unwrap().len(), 1);

        let back: AuditLog = serde_json::from_value(value).unwrap();
        assert_eq!(back, log);
    }
}
