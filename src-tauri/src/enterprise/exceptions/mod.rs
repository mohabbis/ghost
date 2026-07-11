//! Exception classification for enterprise workflow reviews.
//!
//! Exceptions are review signals only; this module never mutates files or
//! advances a run without a separate approval/execution step.

use crate::enterprise::workflows::{ExceptionSeverity, WorkflowException};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExceptionRule {
    pub kind: String,
    pub severity: ExceptionSeverity,
    pub reviewer_action: String,
}

impl ExceptionRule {
    pub fn classify(
        &self,
        exception_id: impl Into<String>,
        message: impl Into<String>,
    ) -> WorkflowException {
        WorkflowException {
            exception_id: exception_id.into(),
            kind: self.kind.clone(),
            severity: self.severity.clone(),
            message: message.into(),
            reviewer_action: self.reviewer_action.clone(),
        }
    }
}

pub fn has_blockers(exceptions: &[WorkflowException]) -> bool {
    exceptions
        .iter()
        .any(|exception| exception.severity == ExceptionSeverity::Blocker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_blocking_exceptions() {
        let rule = ExceptionRule {
            kind: "duplicate_submission".into(),
            severity: ExceptionSeverity::Blocker,
            reviewer_action: "choose one source file before execution".into(),
        };
        assert!(has_blockers(&[rule.classify("ex-1", "same invoice twice")]));
    }
}
