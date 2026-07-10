//! The audit log: an append-only record of what the executor did.
//!
//! Every action the executor touches — applied, skipped, or failed — produces
//! one [`AuditEvent`]. The log is a pure, serializable value (no IO); the
//! executor builds it and the UI/persistence layers read it. This is the
//! "Audit" step of the trust pipeline made concrete and inspectable.

use crate::policy::Capability;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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

/// How an executed action was authorized: without prompting (an `automate`
/// rule) or under the user's explicit approval of the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Automated,
    UserApproved,
}

/// One row in the audit log: the capability, its outcome, the rule that fired,
/// how the action was authorized, and when it happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    /// The action that was considered, expressed as a policy capability.
    pub capability: Capability,
    /// The result of considering it.
    pub outcome: ActionOutcome,
    /// The folder rule (by path) that authorized or refused this action, when
    /// one fired. Absent on rows written before rule attribution existed and
    /// on plain deny-by-default refusals (no rule matched). Optional and
    /// default so old persisted logs keep deserializing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_path: Option<PathBuf>,
    /// How the action was authorized. Recorded for actions that were actually
    /// attempted; absent on skips and on rows written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
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

    /// Append one event, stamped with the current time, with no rule
    /// attribution or provenance (kept for callers predating those fields).
    pub fn record(&mut self, capability: Capability, outcome: ActionOutcome) {
        self.record_attributed(capability, outcome, None, None);
    }

    /// Append one event with the rule that fired and how it was authorized.
    pub fn record_attributed(
        &mut self,
        capability: Capability,
        outcome: ActionOutcome,
        rule_path: Option<PathBuf>,
        provenance: Option<Provenance>,
    ) {
        self.events.push(AuditEvent {
            capability,
            outcome,
            rule_path,
            provenance,
            at: now_ts(),
        });
    }

    /// Render the log as CSV for export: one row per event, with the rule that
    /// fired and the approval provenance alongside the action and outcome.
    pub fn to_csv(&self) -> String {
        let mut out = String::from("at,action,path,target,outcome,detail,rule,provenance\n");
        for event in &self.events {
            let (kind, path, target) = describe_capability(&event.capability);
            let (outcome, detail) = match &event.outcome {
                ActionOutcome::Applied => ("applied", String::new()),
                ActionOutcome::Skipped { reason } => ("skipped", reason.clone()),
                ActionOutcome::Failed { error } => ("failed", error.clone()),
            };
            let rule = event
                .rule_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let provenance = match event.provenance {
                Some(Provenance::Automated) => "automated",
                Some(Provenance::UserApproved) => "user_approved",
                None => "",
            };
            let row = [
                event.at.as_str(),
                kind,
                &path,
                &target,
                outcome,
                &detail,
                &rule,
                provenance,
            ];
            let escaped: Vec<String> = row.iter().map(|f| csv_field(f)).collect();
            out.push_str(&escaped.join(","));
            out.push('\n');
        }
        out
    }

    /// Render the log as a Markdown compliance report: an executive summary
    /// with metrics followed by an itemized tabular ledger of every event,
    /// complete with the run's tamper-evident seal and link.
    pub fn to_compliance_report(
        &self,
        execution_id: &str,
        created_at: &str,
        hash: &str,
        prev_hash: &str,
    ) -> String {
        let mut out = String::new();
        out.push_str("# Ghost Organizer Compliance Report\n\n");

        out.push_str("## Executive Summary\n\n");

        let mut applied = 0;
        let mut skipped = 0;
        let mut failed = 0;
        let mut automated = 0;
        let mut user_approved = 0;

        for event in &self.events {
            match &event.outcome {
                ActionOutcome::Applied => applied += 1,
                ActionOutcome::Skipped { .. } => skipped += 1,
                ActionOutcome::Failed { .. } => failed += 1,
            }
            match event.provenance {
                Some(Provenance::Automated) => automated += 1,
                Some(Provenance::UserApproved) => user_approved += 1,
                None => {}
            }
        }

        let total = self.events.len();

        out.push_str(&format!("* **Execution ID**: `{}`\n", execution_id));
        out.push_str(&format!("* **Timestamp**: `{}`\n", created_at));
        out.push_str(&format!("* **Tamper-Evident Seal (SHA-256)**: `{}`\n", hash));
        out.push_str(&format!("* **Prior Execution Link**: `{}`\n\n", prev_hash));

        out.push_str("### Run Outcome Stats\n\n");
        out.push_str(&format!("* **Total Actions Processed**: {}\n", total));
        out.push_str(&format!("* **Successfully Applied (Moved/Renamed)**: {}\n", applied));
        out.push_str(&format!("* **Skipped**: {}\n", skipped));
        out.push_str(&format!("* **Failed**: {}\n", failed));
        
        let total_prov = automated + user_approved;
        if total_prov > 0 {
            let auto_rate = (automated as f64 / total_prov as f64) * 100.0;
            out.push_str(&format!("* **Automation Rate**: {:.1}%\n\n", auto_rate));
        } else {
            out.push_str("* **Automation Rate**: N/A\n\n");
        }

        out.push_str("## Itemized Audit Ledger\n\n");
        out.push_str("| Timestamp | Action | Source Path | Target / Details | Outcome | Rule | Authorization |\n");
        out.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");

        let escape_md = |s: &str| s.replace('|', "\\|");

        for event in &self.events {
            let (kind, path, target) = describe_capability(&event.capability);
            let outcome_str = match &event.outcome {
                ActionOutcome::Applied => "✅ Applied".to_string(),
                ActionOutcome::Skipped { reason } => format!("⚠️ Skipped ({})", reason),
                ActionOutcome::Failed { error } => format!("❌ Failed ({})", error),
            };
            let rule = event
                .rule_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "N/A".to_string());
            let auth = match event.provenance {
                Some(Provenance::Automated) => "Automated",
                Some(Provenance::UserApproved) => "Approved",
                None => "N/A",
            };

            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                escape_md(&event.at),
                escape_md(kind),
                escape_md(&path),
                escape_md(&target),
                escape_md(&outcome_str),
                escape_md(&rule),
                escape_md(auth)
            ));
        }

        out
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

/// A short action name plus the capability's path(s) for tabular export.
fn describe_capability(cap: &Capability) -> (&'static str, String, String) {
    match cap {
        Capability::ReadFolder { path } => {
            ("read_folder", path.display().to_string(), String::new())
        }
        Capability::CreateFolder { path } => {
            ("create_folder", path.display().to_string(), String::new())
        }
        Capability::RenameFile { from, to } => (
            "rename_file",
            from.display().to_string(),
            to.display().to_string(),
        ),
        Capability::MoveFile { from, to } => (
            "move_file",
            from.display().to_string(),
            to.display().to_string(),
        ),
        Capability::CopyFile { from, to } => (
            "copy_file",
            from.display().to_string(),
            to.display().to_string(),
        ),
        Capability::DeleteFile { path } => {
            ("delete_file", path.display().to_string(), String::new())
        }
        other => ("other", format!("{other:?}"), String::new()),
    }
}

/// Quote a CSV field per RFC 4180: wrap in quotes when it contains a comma,
/// quote, or newline, doubling embedded quotes.
fn csv_field(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
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

    /// Audit rows persisted before rule attribution and provenance existed
    /// must keep deserializing (the fields are optional and defaulted).
    #[test]
    fn old_persisted_events_without_attribution_still_deserialize() {
        let old_json = r#"[{
            "capability": { "kind": "move_file", "from": "/z/a.pdf", "to": "/z/Documents/a.pdf" },
            "outcome": { "outcome": "applied" },
            "at": "1700000000"
        }]"#;
        let log: AuditLog = serde_json::from_str(old_json).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log.events()[0].rule_path, None);
        assert_eq!(log.events()[0].provenance, None);
    }

    /// Attribution fields serialize only when present, so rows without them
    /// keep the exact pre-existing wire shape.
    #[test]
    fn attribution_serializes_only_when_present() {
        let mut log = AuditLog::new();
        log.record(
            Capability::CreateFolder {
                path: PathBuf::from("/z/Documents"),
            },
            ActionOutcome::Applied,
        );
        log.record_attributed(
            Capability::MoveFile {
                from: PathBuf::from("/z/a.pdf"),
                to: PathBuf::from("/z/Documents/a.pdf"),
            },
            ActionOutcome::Applied,
            Some(PathBuf::from("/z")),
            Some(Provenance::UserApproved),
        );
        let value = serde_json::to_value(&log).unwrap();
        let rows = value.as_array().unwrap();
        assert!(rows[0].get("rule_path").is_none());
        assert!(rows[0].get("provenance").is_none());
        assert_eq!(rows[1]["rule_path"], serde_json::json!("/z"));
        assert_eq!(rows[1]["provenance"], serde_json::json!("user_approved"));
    }

    #[test]
    fn to_csv_renders_one_row_per_event_with_escaping() {
        let mut log = AuditLog::new();
        log.record_attributed(
            Capability::MoveFile {
                from: PathBuf::from("/z/a, with comma.pdf"),
                to: PathBuf::from("/z/Documents/a.pdf"),
            },
            ActionOutcome::Skipped {
                reason: "he said \"no\"".into(),
            },
            Some(PathBuf::from("/z")),
            None,
        );
        let csv = log.to_csv();
        let mut lines = csv.lines();
        assert_eq!(
            lines.next().unwrap(),
            "at,action,path,target,outcome,detail,rule,provenance"
        );
        let row = lines.next().unwrap();
        assert!(row.contains("move_file"));
        assert!(
            row.contains("\"/z/a, with comma.pdf\""),
            "comma field must be quoted: {row}"
        );
        assert!(
            row.contains("\"he said \"\"no\"\"\""),
            "quotes must be doubled: {row}"
        );
        assert!(lines.next().is_none());
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

    #[test]
    fn compliance_report_formatting_and_stats() {
        let mut log = AuditLog::new();
        log.record_attributed(
            Capability::MoveFile {
                from: PathBuf::from("/z/a.pdf"),
                to: PathBuf::from("/z/Documents/a.pdf"),
            },
            ActionOutcome::Applied,
            Some(PathBuf::from("/z")),
            Some(Provenance::UserApproved),
        );
        log.record_attributed(
            Capability::DeleteFile {
                path: PathBuf::from("/z/b.pdf"),
            },
            ActionOutcome::Skipped {
                reason: "policy denied".into(),
            },
            None,
            None,
        );

        let report = log.to_compliance_report("test-exec-id", "1700000000", "hash123", "prev456");
        assert!(report.contains("# Ghost Organizer Compliance Report"));
        assert!(report.contains("**Execution ID**: `test-exec-id`"));
        assert!(report.contains("**Timestamp**: `1700000000`"));
        assert!(report.contains("**Tamper-Evident Seal (SHA-256)**: `hash123`"));
        assert!(report.contains("**Prior Execution Link**: `prev456`"));
        assert!(report.contains("Successfully Applied (Moved/Renamed)**: 1"));
        assert!(report.contains("Skipped**: 1"));
        assert!(report.contains("Failed**: 0"));
        assert!(report.contains("a.pdf"));
        assert!(report.contains("b.pdf"));
        assert!(report.contains("Approved"));
        assert!(report.contains("N/A"));
    }
}
