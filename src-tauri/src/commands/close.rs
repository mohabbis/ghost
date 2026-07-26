//! Month-end close commands: read-only reconciliation preview over file names.
//!
//! Risk class: **safe-read**. This command touches no filesystem, no network,
//! no OS input, and no secrets — it classifies the file *names* the caller
//! passes in and reconciles them against a caller-supplied checklist, returning
//! a report. Acting on that report (filing the documents, sealing a signed-off
//! close) still flows through the Organizer's
//! preview -> approve -> execute -> audit -> undo pipeline; this command only
//! reports.

use crate::filing::finance::classify_report;
use crate::finance::close::{CloseChecklist, PresentDoc, ReconcileReport, reconcile};

/// Reconcile a batch of file names against a month-end close checklist.
///
/// Read-only: each name is classified by [`classify_report`] (name-only, never
/// reads the disk), so it is safe to run on a paste-in list before any folder
/// access is granted. Names that classify to a financial document become
/// [`PresentDoc`]s; names that don't classify are ignored. The returned
/// [`ReconcileReport`] lists each requirement as present/partial/missing plus
/// any documents present but not on the checklist. It mutates nothing and
/// decides nothing — surplus documents are surfaced for the human, never
/// resolved automatically.
#[tauri::command]
pub fn preview_close_reconcile(
    checklist: CloseChecklist,
    file_names: Vec<String>,
) -> ReconcileReport {
    let present: Vec<PresentDoc> = file_names
        .iter()
        .filter_map(|name| {
            classify_report(name).map(|c| PresentDoc {
                file_name: name.clone(),
                kind: c.kind,
            })
        })
        .collect();
    reconcile(&checklist, &present)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filing::finance::ReportKind;
    use crate::finance::close::{ExpectedDoc, ReconcileStatus};

    fn checklist(expected: Vec<ExpectedDoc>) -> CloseChecklist {
        CloseChecklist {
            client: "Acme".to_string(),
            period: None,
            expected,
        }
    }

    #[test]
    fn classifies_names_and_reports_requirement_met() {
        let report = preview_close_reconcile(
            checklist(vec![ExpectedDoc::one(ReportKind::BankStatement)]),
            vec!["June 2026 bank statement.pdf".to_string()],
        );
        assert_eq!(report.requirements.len(), 1);
        assert_eq!(report.requirements[0].status, ReconcileStatus::Present);
        assert!(report.complete);
    }

    #[test]
    fn missing_requirement_is_reported_not_blocked() {
        let report = preview_close_reconcile(
            checklist(vec![ExpectedDoc::one(ReportKind::Payroll)]),
            vec!["June 2026 bank statement.pdf".to_string()],
        );
        assert_eq!(report.requirements[0].status, ReconcileStatus::Missing);
        assert!(!report.complete);
    }

    #[test]
    fn unclassifiable_names_are_ignored() {
        // A photo carries no financial signal, so it must not appear as a
        // present or an unexpected document — only the bank statement does.
        let report = preview_close_reconcile(
            checklist(vec![ExpectedDoc::one(ReportKind::BankStatement)]),
            vec![
                "June 2026 bank statement.pdf".to_string(),
                "IMG_4821.HEIC".to_string(),
            ],
        );
        assert!(report.complete);
        assert!(
            report.unexpected.is_empty(),
            "non-financial files must not be surfaced as unexpected docs"
        );
    }
}
