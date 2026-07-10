//! Filing-preview commands: audience-aware, read-only planning over file names.
//!
//! Risk class: **safe-read**. These commands touch no filesystem, no network,
//! no OS input, and no secrets — they reason purely over the file *names* the
//! caller passes in and return a proposed structure plus a savings estimate.
//! The actual move/rename still flows through the Organizer's
//! preview -> approve -> execute -> audit -> undo pipeline.

use crate::filing::{
    estimate_savings, preview_filing, Audience, FilingPreview, SavingsEstimate, SavingsInputs,
};

/// Propose where a batch of files would be filed under an audience profile.
///
/// Read-only: operates only on the provided `file_names` (never reads the disk),
/// so it is safe to run on a paste-in list before any folder access is granted.
/// `root` is the destination folder name; pass an empty string for the
/// audience default (e.g. "Financial Reports", "Coursework").
#[tauri::command]
pub fn preview_file_filing(
    audience: Audience,
    root: Option<String>,
    file_names: Vec<String>,
) -> FilingPreview {
    preview_filing(audience, root.as_deref().unwrap_or(""), &file_names)
}

/// Estimate annual time (and, if an hourly rate is given, cost) saved by moving
/// a repetitive manual filing task to a reviewed, assisted workflow.
///
/// Pure arithmetic over the supplied inputs; no IO. The returned estimate
/// echoes back every default it applied so the figure is auditable.
#[tauri::command]
pub fn estimate_filing_savings(inputs: SavingsInputs) -> SavingsEstimate {
    estimate_savings(&inputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_command_uses_audience_default_root() {
        let preview = preview_file_filing(
            Audience::Finance,
            None,
            vec!["Q2 2026 Income Statement.xlsx".to_string()],
        );
        assert_eq!(preview.root, "Financial Reports");
        assert_eq!(preview.recognized, 1);
    }

    #[test]
    fn preview_command_respects_explicit_root() {
        let preview = preview_file_filing(
            Audience::Student,
            Some("School/2026".to_string()),
            vec!["CS101 homework 3.pdf".to_string()],
        );
        assert_eq!(preview.root, "School/2026");
    }

    #[test]
    fn savings_command_returns_estimate() {
        let est = estimate_filing_savings(SavingsInputs {
            files_per_period: 10.0,
            periods_per_year: 12.0,
            minutes_per_file_manual: 6.0,
            minutes_per_file_assisted: None,
            hourly_rate: Some(40.0),
            manual_error_rate: None,
            minutes_per_error_rework: None,
        });
        assert!(est.hours_saved_per_year > 0.0);
        assert!(est.cost_saved_per_year.is_some());
    }
}
