//! Deterministic month-end close reconciliation.
//!
//! Pure, local-first: no network, no AI, no mutation. Given a per-client
//! *expected-documents checklist* and the documents already classified as
//! present for a period, [`reconcile`] reports which requirements are met,
//! which are missing or only partially satisfied, and which present documents
//! were not expected. The verdict is arithmetic over classified document
//! kinds — not a model's judgement — so the same inputs always produce the
//! same report.
//!
//! This module only *reports*. It never deletes a "duplicate", moves a file,
//! or decides a close is signed off. Acting on a report (filing, producing a
//! signable close packet) stays a separate, policy-gated, audited, undoable
//! step outside this module — the same boundary the rest of `finance/` keeps.
//!
//! Classification of a file name into a [`ReportKind`] is done upstream by
//! [`crate::filing::finance::classify_report`]; this module takes the already
//! classified kind as input and counts.

use crate::filing::Period;
use crate::filing::finance::ReportKind;
use serde::{Deserialize, Serialize};

/// One expected-document requirement in a close checklist: at least
/// `min_count` documents of `kind` must be present for the period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedDoc {
    pub kind: ReportKind,
    /// Minimum number of documents of this kind required (typically 1).
    pub min_count: u32,
}

impl ExpectedDoc {
    /// Require at least one document of `kind`.
    pub fn one(kind: ReportKind) -> Self {
        Self { kind, min_count: 1 }
    }
}

/// A document already classified as present for the client/period. Read-only
/// input; the reconcile step never reads file contents or the network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentDoc {
    pub file_name: String,
    pub kind: ReportKind,
}

/// The per-client expected-documents checklist for one close period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseChecklist {
    pub client: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    pub expected: Vec<ExpectedDoc>,
}

impl CloseChecklist {
    /// A conservative starting checklist for a monthly bookkeeping close: at
    /// least one bank statement for the period. Deliberately minimal — a firm
    /// tunes the requirements to its own process; this is only a safe default
    /// so the reconcile step is usable out of the box.
    pub fn monthly_bookkeeping(client: impl Into<String>, period: Option<Period>) -> Self {
        Self {
            client: client.into(),
            period,
            expected: vec![ExpectedDoc::one(ReportKind::BankStatement)],
        }
    }
}

/// Whether a single requirement was met by the present documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconcileStatus {
    /// `found >= required` (a `required` of 0 is trivially present).
    Present,
    /// Some documents of the kind are present, but fewer than required.
    Partial,
    /// None of the kind are present, and at least one was required.
    Missing,
}

/// The outcome for one expected-document requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementResult {
    pub kind: ReportKind,
    pub required: u32,
    pub found: u32,
    pub status: ReconcileStatus,
    /// File names of the present documents that matched this requirement's
    /// kind, in input order. `found` above may exceed `required` — surplus is
    /// surfaced (a possible duplicate) for the human, never auto-resolved.
    pub matched_files: Vec<String>,
}

/// The reconciliation report for one client/period. Pure data for a review
/// surface — it mutates nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileReport {
    pub client: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    pub requirements: Vec<RequirementResult>,
    /// Present documents whose kind is not named by any requirement.
    pub unexpected: Vec<PresentDoc>,
    /// Number of requirements in the checklist.
    pub required_total: u32,
    /// Number of requirements met (`ReconcileStatus::Present`).
    pub satisfied: u32,
    /// True when every requirement is met. Note: `unexpected` documents do
    /// **not** make a close incomplete — they are surfaced, not blocking.
    pub complete: bool,
}

/// Reconcile the documents `present` for a client/period against the
/// `checklist`. Deterministic: iterates the checklist in order, counts present
/// documents by kind, and reports status per requirement plus any unexpected
/// documents. Blocks nothing and changes nothing.
pub fn reconcile(checklist: &CloseChecklist, present: &[PresentDoc]) -> ReconcileReport {
    let mut requirements = Vec::with_capacity(checklist.expected.len());
    let mut satisfied = 0u32;

    for req in &checklist.expected {
        let matched_files: Vec<String> = present
            .iter()
            .filter(|d| d.kind == req.kind)
            .map(|d| d.file_name.clone())
            .collect();
        let found = matched_files.len() as u32;

        let status = if found >= req.min_count {
            ReconcileStatus::Present
        } else if found > 0 {
            ReconcileStatus::Partial
        } else {
            ReconcileStatus::Missing
        };
        if status == ReconcileStatus::Present {
            satisfied += 1;
        }

        requirements.push(RequirementResult {
            kind: req.kind,
            required: req.min_count,
            found,
            status,
            matched_files,
        });
    }

    // A present document is "unexpected" when its kind is named by no
    // requirement in the checklist.
    let unexpected: Vec<PresentDoc> = present
        .iter()
        .filter(|d| !checklist.expected.iter().any(|req| req.kind == d.kind))
        .cloned()
        .collect();

    let required_total = checklist.expected.len() as u32;
    ReconcileReport {
        client: checklist.client.clone(),
        period: checklist.period,
        requirements,
        unexpected,
        required_total,
        satisfied,
        complete: satisfied == required_total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(name: &str, kind: ReportKind) -> PresentDoc {
        PresentDoc {
            file_name: name.to_string(),
            kind,
        }
    }

    fn checklist(expected: Vec<ExpectedDoc>) -> CloseChecklist {
        CloseChecklist {
            client: "Acme".to_string(),
            period: None,
            expected,
        }
    }

    #[test]
    fn requirement_met_is_present_and_complete() {
        let cl = checklist(vec![ExpectedDoc::one(ReportKind::BankStatement)]);
        let present = vec![doc("2026-06_stmt.pdf", ReportKind::BankStatement)];
        let report = reconcile(&cl, &present);

        assert_eq!(report.requirements.len(), 1);
        assert_eq!(report.requirements[0].status, ReconcileStatus::Present);
        assert_eq!(report.requirements[0].found, 1);
        assert_eq!(
            report.requirements[0].matched_files,
            vec!["2026-06_stmt.pdf"]
        );
        assert_eq!(report.satisfied, 1);
        assert_eq!(report.required_total, 1);
        assert!(report.complete);
        assert!(report.unexpected.is_empty());
    }

    #[test]
    fn missing_requirement_is_flagged_not_blocked() {
        let cl = checklist(vec![
            ExpectedDoc::one(ReportKind::BankStatement),
            ExpectedDoc::one(ReportKind::Payroll),
        ]);
        let present = vec![doc("stmt.pdf", ReportKind::BankStatement)];
        let report = reconcile(&cl, &present);

        assert_eq!(report.requirements[0].status, ReconcileStatus::Present);
        assert_eq!(report.requirements[1].status, ReconcileStatus::Missing);
        assert_eq!(report.requirements[1].found, 0);
        assert!(report.requirements[1].matched_files.is_empty());
        assert_eq!(report.satisfied, 1);
        assert!(!report.complete);
    }

    #[test]
    fn partial_when_fewer_than_required() {
        let cl = checklist(vec![ExpectedDoc {
            kind: ReportKind::BankStatement,
            min_count: 2,
        }]);
        let present = vec![doc("stmt.pdf", ReportKind::BankStatement)];
        let report = reconcile(&cl, &present);

        assert_eq!(report.requirements[0].status, ReconcileStatus::Partial);
        assert_eq!(report.requirements[0].found, 1);
        assert_eq!(report.requirements[0].required, 2);
        assert!(!report.complete);
    }

    #[test]
    fn surplus_documents_still_present_and_counted() {
        let cl = checklist(vec![ExpectedDoc::one(ReportKind::Invoice)]);
        let present = vec![
            doc("inv1.pdf", ReportKind::Invoice),
            doc("inv2.pdf", ReportKind::Invoice),
            doc("inv3.pdf", ReportKind::Invoice),
        ];
        let report = reconcile(&cl, &present);

        assert_eq!(report.requirements[0].status, ReconcileStatus::Present);
        assert_eq!(report.requirements[0].found, 3);
        assert_eq!(report.requirements[0].matched_files.len(), 3);
        assert!(report.complete);
    }

    #[test]
    fn unexpected_documents_are_surfaced_but_do_not_block() {
        let cl = checklist(vec![ExpectedDoc::one(ReportKind::BankStatement)]);
        let present = vec![
            doc("stmt.pdf", ReportKind::BankStatement),
            doc("random_receipt.pdf", ReportKind::Expense),
        ];
        let report = reconcile(&cl, &present);

        assert!(
            report.complete,
            "unexpected docs must not make a close incomplete"
        );
        assert_eq!(report.unexpected.len(), 1);
        assert_eq!(report.unexpected[0].file_name, "random_receipt.pdf");
        assert_eq!(report.unexpected[0].kind, ReportKind::Expense);
    }

    #[test]
    fn zero_min_count_is_trivially_present() {
        let cl = checklist(vec![ExpectedDoc {
            kind: ReportKind::Payroll,
            min_count: 0,
        }]);
        let report = reconcile(&cl, &[]);

        assert_eq!(report.requirements[0].status, ReconcileStatus::Present);
        assert!(report.complete);
    }

    #[test]
    fn empty_checklist_is_complete_with_all_present_unexpected() {
        let cl = checklist(vec![]);
        let present = vec![doc("stmt.pdf", ReportKind::BankStatement)];
        let report = reconcile(&cl, &present);

        assert_eq!(report.required_total, 0);
        assert_eq!(report.satisfied, 0);
        assert!(report.complete);
        assert_eq!(report.unexpected.len(), 1);
    }

    #[test]
    fn monthly_bookkeeping_preset_requires_a_bank_statement() {
        let cl = CloseChecklist::monthly_bookkeeping("Borealis", None);
        let empty = reconcile(&cl, &[]);
        assert!(!empty.complete);
        assert_eq!(empty.requirements[0].status, ReconcileStatus::Missing);

        let with_stmt = reconcile(&cl, &[doc("june.pdf", ReportKind::BankStatement)]);
        assert!(with_stmt.complete);
    }

    #[test]
    fn report_is_deterministic() {
        let cl = CloseChecklist::monthly_bookkeeping("Acme", None);
        let present = vec![
            doc("a.pdf", ReportKind::BankStatement),
            doc("b.pdf", ReportKind::Invoice),
        ];
        assert_eq!(reconcile(&cl, &present), reconcile(&cl, &present));
    }
}
