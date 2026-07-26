//! Month-end close commands: read-only reconciliation.
//!
//! Risk class: **safe-read**. Both commands only *report* — they classify
//! document names and reconcile them against a caller-supplied checklist,
//! mutating nothing and deciding nothing. [`preview_close_reconcile`] is
//! name-only (no disk access at all); [`close_reconcile_zone`] reads a Zone's
//! readable source folders, but only directory *metadata* (names), never file
//! contents, and only inside folders the Zone's rules grant read access to —
//! the same footprint as `organizer_scanner_inbox_signal`. No network, no OS
//! input, no secrets. Acting on a report (filing the documents, sealing a
//! signed-off close) still flows through the Organizer's
//! preview -> approve -> execute -> audit -> undo pipeline.

use crate::filing::finance::classify_report;
use crate::finance::close::{CloseChecklist, PresentDoc, ReconcileReport, reconcile};
use crate::organizer::planner::scan_zone_files;
use crate::storage::zones::get_zone;
use crate::storage::{Db, open_default};

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

/// Reconcile the documents actually sitting in a Zone against a close checklist.
///
/// Read-only: scans the Zone's readable source folders for file *names*
/// (metadata only, never contents — see [`scan_zone_files`]), classifies each,
/// and reconciles against `checklist`. Only folders the Zone's rules grant read
/// access to are scanned, so the footprint matches `organizer_scanner_inbox_signal`.
/// Reports only — it mutates nothing; filing/sign-off stays in the audited,
/// undoable executor.
#[tauri::command]
pub fn close_reconcile_zone(
    zone_id: String,
    checklist: CloseChecklist,
) -> Result<ReconcileReport, String> {
    let conn = open_default().map_err(|e| e.to_string())?;
    close_reconcile_zone_with_conn(&conn, &zone_id, &checklist).map_err(|e| e.to_string())
}

/// Testable core of [`close_reconcile_zone`], split out so it can run against an
/// in-memory database (`open_default` has no test seam).
pub(crate) fn close_reconcile_zone_with_conn(
    conn: &Db,
    zone_id: &str,
    checklist: &CloseChecklist,
) -> anyhow::Result<ReconcileReport> {
    get_zone(conn, zone_id)?.ok_or_else(|| anyhow::anyhow!("no zone with id {zone_id}"))?;
    let present: Vec<PresentDoc> = scan_zone_files(conn, zone_id)?
        .iter()
        .filter_map(|f| {
            classify_report(&f.file_name).map(|c| PresentDoc {
                file_name: f.file_name.clone(),
                kind: c.kind,
            })
        })
        .collect();
    Ok(reconcile(checklist, &present))
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

    // ── close_reconcile_zone ───────────────────────────────────────────────
    use crate::policy::zone::{DefaultDecision, FolderRule};
    use crate::storage::open_in_memory;
    use crate::storage::zones::create_zone_with_rules;

    // Create a Zone whose one read-only folder rule points at `dir`.
    fn zone_over(conn: &Db, dir: &std::path::Path) -> String {
        create_zone_with_rules(
            conn,
            "close",
            None,
            DefaultDecision::Ask,
            false,
            &[FolderRule::read_only(dir.to_path_buf())],
        )
        .unwrap()
        .id
    }

    #[test]
    fn zone_scan_reconciles_present_documents() {
        let conn = open_in_memory().unwrap();
        let tmp = crate::organizer::testutil::tempdir();
        tmp.file("June 2026 bank statement.pdf", b"x");
        let zone_id = zone_over(&conn, tmp.path());

        let report = close_reconcile_zone_with_conn(
            &conn,
            &zone_id,
            &checklist(vec![ExpectedDoc::one(ReportKind::BankStatement)]),
        )
        .unwrap();

        assert_eq!(report.requirements[0].status, ReconcileStatus::Present);
        assert!(report.complete);
    }

    #[test]
    fn zone_scan_reports_missing_when_folder_empty() {
        let conn = open_in_memory().unwrap();
        let tmp = crate::organizer::testutil::tempdir();
        let zone_id = zone_over(&conn, tmp.path());

        let report = close_reconcile_zone_with_conn(
            &conn,
            &zone_id,
            &checklist(vec![ExpectedDoc::one(ReportKind::BankStatement)]),
        )
        .unwrap();

        assert_eq!(report.requirements[0].status, ReconcileStatus::Missing);
        assert!(!report.complete);
    }

    #[test]
    fn zone_scan_unknown_zone_is_error() {
        let conn = open_in_memory().unwrap();
        let err = close_reconcile_zone_with_conn(
            &conn,
            "does-not-exist",
            &checklist(vec![ExpectedDoc::one(ReportKind::BankStatement)]),
        );
        assert!(err.is_err());
    }
}
