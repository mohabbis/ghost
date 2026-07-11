//! Audience-agnostic **filing preview**: given a list of file names and an
//! audience profile, propose where each file would be filed.
//!
//! This is a pure, read-only planning step — the "Propose plan" stage of the
//! trust pipeline. It touches **no filesystem**: it reasons only over the file
//! names it is handed, so it is safe to run on a paste-in list before the user
//! has granted any folder access. The actual move/rename still goes through the
//! Organizer's preview -> approve -> execute -> audit -> undo path.

use super::{academic, engineering, finance, period};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Which domain profile to apply when proposing a filing structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    /// Finance/operations: recurring financial reports by type + period.
    Finance,
    /// Students: coursework by course + term + assignment type.
    Student,
    /// Software teams: automation artifacts by type + run period.
    Engineering,
}

impl Audience {
    fn default_root(self) -> &'static str {
        match self {
            Audience::Finance => "Financial Reports",
            Audience::Student => "Coursework",
            Audience::Engineering => "Engineering Automation",
        }
    }
}

/// One file's proposed filing outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FiledItem {
    pub file_name: String,
    /// Human-readable category (report type / coursework type), or "Unsorted".
    pub category: String,
    /// Human-readable period/term label, when one was detected.
    pub period: Option<String>,
    /// Proposed destination directory, relative to the chosen root, using `/`.
    pub relative_dir: String,
    pub confidence: f32,
    pub reason: String,
    /// Whether the profile recognized the file at all.
    pub recognized: bool,
    /// Whether the item should be surfaced for manual review before filing.
    pub needs_review: bool,
}

/// The full preview across a batch of files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilingPreview {
    pub audience: Audience,
    pub root: String,
    pub items: Vec<FiledItem>,
    pub recognized: usize,
    pub needs_review: usize,
    pub unrecognized: usize,
    pub distinct_categories: usize,
    pub distinct_periods: usize,
}

const UNSORTED_DIR: &str = "Needs Review";

/// Build a filing preview for `file_names` under `audience`'s rules.
///
/// `root` overrides the audience's default destination folder name; pass an
/// empty string to use the default. Pure and deterministic.
pub fn preview_filing(audience: Audience, root: &str, file_names: &[String]) -> FilingPreview {
    let root = if root.trim().is_empty() {
        audience.default_root().to_string()
    } else {
        root.trim().to_string()
    };

    let mut items = Vec::with_capacity(file_names.len());
    let mut categories = BTreeSet::new();
    let mut periods = BTreeSet::new();
    let mut recognized = 0usize;
    let mut needs_review = 0usize;
    let mut unrecognized = 0usize;

    for name in file_names {
        let item = match audience {
            Audience::Finance => file_finance(&root, name),
            Audience::Student => file_student(&root, name),
            Audience::Engineering => file_engineering(&root, name),
        };
        if item.recognized {
            recognized += 1;
            categories.insert(item.category.clone());
            if let Some(p) = &item.period {
                periods.insert(p.clone());
            }
        } else {
            unrecognized += 1;
        }
        if item.needs_review {
            needs_review += 1;
        }
        items.push(item);
    }

    FilingPreview {
        audience,
        root,
        items,
        recognized,
        needs_review,
        unrecognized,
        distinct_categories: categories.len(),
        distinct_periods: periods.len(),
    }
}

fn stem_of(file_name: &str) -> String {
    std::path::Path::new(file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| file_name.to_string())
}

fn join_dir(segments: &[String]) -> String {
    segments
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("/")
}

fn file_finance(root: &str, name: &str) -> FiledItem {
    match finance::classify_report(name) {
        Some(c) => {
            let period = period::extract_period(&stem_of(name));
            let mut segments = vec![root.to_string(), c.kind.folder_name().to_string()];
            match &period {
                Some(p) => segments.extend(p.dir_segments()),
                None => segments.push("Undated".to_string()),
            }
            let needs_review = c.confidence <= finance::LOW_CONFIDENCE || period.is_none();
            FiledItem {
                file_name: name.to_string(),
                category: c.kind.label().to_string(),
                period: period.map(|p| p.label()),
                relative_dir: join_dir(&segments),
                confidence: c.confidence,
                reason: c.reason,
                recognized: true,
                needs_review,
            }
        }
        None => unsorted(root, name),
    }
}

fn file_student(root: &str, name: &str) -> FiledItem {
    match academic::classify_coursework(name) {
        Some(c) => {
            let term = academic::extract_term(name);
            let period = period::extract_period(&stem_of(name));
            let (period_label, period_dir) = match (term, period) {
                (Some(t), _) => (Some(t.label()), Some(t.dir_name())),
                (None, Some(p)) => (Some(p.label()), Some(join_dir(&p.dir_segments()))),
                (None, None) => (None, None),
            };
            let mut segments = vec![root.to_string()];
            if let Some(course) = &c.course {
                segments.push(course.clone());
            }
            segments.push(c.kind.folder_name().to_string());
            match &period_dir {
                Some(d) => segments.push(d.clone()),
                None => segments.push("Undated".to_string()),
            }
            let needs_review = c.confidence <= academic::LOW_CONFIDENCE;
            FiledItem {
                file_name: name.to_string(),
                category: c.kind.label().to_string(),
                period: period_label,
                relative_dir: join_dir(&segments),
                confidence: c.confidence,
                reason: c.reason,
                recognized: true,
                needs_review,
            }
        }
        None => unsorted(root, name),
    }
}

fn file_engineering(root: &str, name: &str) -> FiledItem {
    match engineering::classify_artifact(name) {
        Some(c) => {
            let period = period::extract_period(&stem_of(name));
            let mut segments = vec![root.to_string(), c.kind.folder_name().to_string()];
            match &period {
                Some(p) => segments.extend(p.dir_segments()),
                None => segments.push("Undated".to_string()),
            }
            let needs_review = c.confidence <= engineering::LOW_CONFIDENCE || period.is_none();
            FiledItem {
                file_name: name.to_string(),
                category: c.kind.label().to_string(),
                period: period.map(|p| p.label()),
                relative_dir: join_dir(&segments),
                confidence: c.confidence,
                reason: c.reason,
                recognized: true,
                needs_review,
            }
        }
        None => unsorted(root, name),
    }
}

fn unsorted(root: &str, name: &str) -> FiledItem {
    FiledItem {
        file_name: name.to_string(),
        category: "Unsorted".to_string(),
        period: None,
        relative_dir: join_dir(&[root.to_string(), UNSORTED_DIR.to_string()]),
        confidence: 0.0,
        reason: "no profile rule recognized this file".to_string(),
        recognized: false,
        needs_review: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn finance_preview_files_by_type_and_period() {
        let preview = preview_filing(
            Audience::Finance,
            "",
            &names(&[
                "Q2 2026 Income Statement.xlsx",
                "Balance Sheet 2026-06.xlsx",
            ]),
        );
        assert_eq!(preview.root, "Financial Reports");
        assert_eq!(preview.recognized, 2);
        assert_eq!(preview.unrecognized, 0);
        assert_eq!(
            preview.items[0].relative_dir,
            "Financial Reports/Income Statements/2026/Q2"
        );
        assert_eq!(
            preview.items[1].relative_dir,
            "Financial Reports/Balance Sheets/2026/2026-06"
        );
    }

    #[test]
    fn finance_undated_report_needs_review() {
        let preview = preview_filing(Audience::Finance, "", &names(&["Balance Sheet.xlsx"]));
        assert!(preview.items[0].recognized);
        assert!(preview.items[0].needs_review);
        assert!(preview.items[0].relative_dir.ends_with("Undated"));
    }

    #[test]
    fn student_preview_files_by_course_type_and_term() {
        let preview = preview_filing(
            Audience::Student,
            "",
            &names(&["CS101 Fall 2026 homework 3.pdf"]),
        );
        assert_eq!(preview.root, "Coursework");
        let item = &preview.items[0];
        assert_eq!(item.category, "Assignment");
        assert_eq!(item.period.as_deref(), Some("Fall 2026"));
        assert_eq!(item.relative_dir, "Coursework/CS101/Assignments/2026 Fall");
    }

    #[test]
    fn engineering_preview_files_by_artifact_type_and_period() {
        let preview = preview_filing(
            Audience::Engineering,
            "",
            &names(&["junit-test-results-2026-07.xml", "coverage-Q2-2026.html"]),
        );
        assert_eq!(preview.root, "Engineering Automation");
        assert_eq!(preview.recognized, 2);
        assert_eq!(preview.items[0].category, "Test Report");
        assert_eq!(
            preview.items[0].relative_dir,
            "Engineering Automation/Test Reports/2026/2026-07"
        );
        assert_eq!(
            preview.items[1].relative_dir,
            "Engineering Automation/Coverage Reports/2026/Q2"
        );
    }

    #[test]
    fn custom_root_is_respected() {
        let preview = preview_filing(
            Audience::Finance,
            "Accounting/2026",
            &names(&["Trial Balance 2026-06.csv"]),
        );
        assert!(preview.items[0]
            .relative_dir
            .starts_with("Accounting/2026/"));
    }

    #[test]
    fn unrecognized_files_are_flagged_for_review() {
        let preview = preview_filing(Audience::Finance, "", &names(&["vacation.jpg"]));
        assert_eq!(preview.unrecognized, 1);
        assert_eq!(preview.recognized, 0);
        assert!(!preview.items[0].recognized);
        assert!(preview.items[0].needs_review);
        assert!(preview.items[0].relative_dir.ends_with("Needs Review"));
    }

    #[test]
    fn preview_counts_distinct_categories_and_periods() {
        let preview = preview_filing(
            Audience::Finance,
            "",
            &names(&[
                "Q1 2026 Income Statement.xlsx",
                "Q2 2026 Income Statement.xlsx",
                "Q1 2026 Balance Sheet.xlsx",
            ]),
        );
        assert_eq!(preview.recognized, 3);
        assert_eq!(preview.distinct_categories, 2); // income statement + balance sheet
        assert_eq!(preview.distinct_periods, 2); // Q1 2026 + Q2 2026
    }

    #[test]
    fn preview_is_deterministic() {
        let files = names(&["Q2 2026 Income Statement.xlsx"]);
        assert_eq!(
            preview_filing(Audience::Finance, "", &files),
            preview_filing(Audience::Finance, "", &files)
        );
    }
}
