//! Student profile: classification of coursework files by assignment type and
//! (where present) course code and academic term.
//!
//! Rules first, AI never (for this layer): a pure function of the file *name*
//! and extension. No network, no model, no IO, no file-content reads. This is
//! the student-audience counterpart to [`super::finance`]; the term/date is
//! parsed by [`super::period`], and the academic *term* (e.g. "Fall 2026") is
//! parsed here since it is domain-specific.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// A coursework artifact type Ghost recognizes by filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CourseworkKind {
    Assignment,
    Lecture,
    Lab,
    Exam,
    Reading,
    Project,
    Notes,
    Syllabus,
    Presentation,
    /// Recognized as coursework, but the specific type is unclear.
    Generic,
}

impl CourseworkKind {
    pub fn folder_name(self) -> &'static str {
        match self {
            CourseworkKind::Assignment => "Assignments",
            CourseworkKind::Lecture => "Lectures",
            CourseworkKind::Lab => "Labs",
            CourseworkKind::Exam => "Exams",
            CourseworkKind::Reading => "Readings",
            CourseworkKind::Project => "Projects",
            CourseworkKind::Notes => "Notes",
            CourseworkKind::Syllabus => "Syllabus",
            CourseworkKind::Presentation => "Presentations",
            CourseworkKind::Generic => "Coursework",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CourseworkKind::Assignment => "Assignment",
            CourseworkKind::Lecture => "Lecture",
            CourseworkKind::Lab => "Lab",
            CourseworkKind::Exam => "Exam",
            CourseworkKind::Reading => "Reading",
            CourseworkKind::Project => "Project",
            CourseworkKind::Notes => "Notes",
            CourseworkKind::Syllabus => "Syllabus",
            CourseworkKind::Presentation => "Presentation",
            CourseworkKind::Generic => "Coursework",
        }
    }
}

/// The academic term a file belongs to, e.g. "Fall 2026".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Term {
    Fall(i32),
    Spring(i32),
    Summer(i32),
    Winter(i32),
}

impl Term {
    pub fn label(self) -> String {
        match self {
            Term::Fall(y) => format!("Fall {y}"),
            Term::Spring(y) => format!("Spring {y}"),
            Term::Summer(y) => format!("Summer {y}"),
            Term::Winter(y) => format!("Winter {y}"),
        }
    }

    /// Folder-safe name for filing, e.g. `"2026 Fall"` (year first keeps terms
    /// chronological when sorted).
    pub fn dir_name(self) -> String {
        match self {
            Term::Fall(y) => format!("{y} Fall"),
            Term::Spring(y) => format!("{y} Spring"),
            Term::Summer(y) => format!("{y} Summer"),
            Term::Winter(y) => format!("{y} Winter"),
        }
    }
}

/// The outcome of recognizing one file as coursework.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CourseworkClassification {
    pub kind: CourseworkKind,
    /// A course code like "CS101" / "BIO 210" when the filename carries one.
    pub course: Option<String>,
    pub confidence: f32,
    pub reason: String,
}

pub const LOW_CONFIDENCE: f32 = 0.5;
const SPECIFIC_CONFIDENCE: f32 = 0.9;
const GENERIC_CONFIDENCE: f32 = 0.55;

/// Extensions that plausibly hold coursework.
const COURSEWORK_EXTS: &[&str] = &[
    "pdf", "doc", "docx", "odt", "rtf", "txt", "md", "tex", "pages", "ppt", "pptx", "odp", "key",
    "xls", "xlsx", "csv", "ipynb", "py", "java", "c", "cpp", "zip",
];

#[allow(clippy::type_complexity)]
fn keyword_table() -> &'static [(&'static [&'static str], CourseworkKind)] {
    &[
        (
            &["syllabus", "course outline", "course info"],
            CourseworkKind::Syllabus,
        ),
        (
            &[
                "assignment",
                "homework",
                "hw",
                "problem set",
                "pset",
                "worksheet",
            ],
            CourseworkKind::Assignment,
        ),
        (&["lab", "laboratory"], CourseworkKind::Lab),
        (
            &[
                "exam",
                "midterm",
                "final exam",
                "finals",
                "quiz",
                "test",
                "practice exam",
            ],
            CourseworkKind::Exam,
        ),
        (
            &["lecture", "slides deck", "class notes"],
            CourseworkKind::Lecture,
        ),
        (
            &["reading", "chapter", "textbook", "paper", "article"],
            CourseworkKind::Reading,
        ),
        (
            &["project", "capstone", "thesis", "dissertation"],
            CourseworkKind::Project,
        ),
        (&["notes"], CourseworkKind::Notes),
        (
            &["presentation", "slides", "deck"],
            CourseworkKind::Presentation,
        ),
    ]
}

const GENERIC_HINTS: &[&str] = &[
    "course",
    "class",
    "semester",
    "professor",
    "lecture",
    "study",
    "school",
    "university",
    "college",
];

fn is_coursework_ext(ext: &str) -> bool {
    COURSEWORK_EXTS.contains(&ext)
}

fn split_name(file_name: &str) -> (String, Option<String>) {
    let path = std::path::Path::new(file_name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    (stem, ext)
}

fn normalize(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len());
    let mut last_space = false;
    for c in stem.chars() {
        let mapped = if matches!(c, '_' | '-' | '.') { ' ' } else { c };
        if mapped == ' ' {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(mapped);
            last_space = false;
        }
    }
    out.trim().to_string()
}

/// Classify a file name as coursework, or `None` when nothing suggests it.
pub fn classify_coursework(file_name: &str) -> Option<CourseworkClassification> {
    let (stem, ext) = split_name(file_name);
    match ext.as_deref() {
        Some(e) if is_coursework_ext(e) => {}
        _ => return None,
    }

    let haystack = normalize(&stem);
    let course = extract_course_code(&haystack);

    for (needles, kind) in keyword_table() {
        if let Some(hit) = needles.iter().find(|n| word_contains(&haystack, n)) {
            return Some(CourseworkClassification {
                kind: *kind,
                course,
                confidence: SPECIFIC_CONFIDENCE,
                reason: format!("matched \"{hit}\" -> {}", kind.label()),
            });
        }
    }

    // A bare course code (e.g. "CS101 handout") is enough to treat as coursework.
    if course.is_some() {
        return Some(CourseworkClassification {
            kind: CourseworkKind::Generic,
            course,
            confidence: GENERIC_CONFIDENCE,
            reason: "carries a course code".to_string(),
        });
    }

    if let Some(hit) = GENERIC_HINTS.iter().find(|n| word_contains(&haystack, n)) {
        return Some(CourseworkClassification {
            kind: CourseworkKind::Generic,
            course,
            confidence: GENERIC_CONFIDENCE,
            reason: format!("matched general coursework term \"{hit}\""),
        });
    }

    None
}

/// Whether `needle` appears in `haystack` bounded by non-alphanumeric edges, so
/// short tokens like "hw" or "lab" don't match inside unrelated words
/// ("However", "label"). `haystack` is already space-normalized.
fn word_contains(haystack: &str, needle: &str) -> bool {
    if needle.contains(' ') {
        return haystack.contains(needle);
    }
    haystack.split(|c: char| !c.is_alphanumeric()).any(|w| {
        if w == needle {
            return true;
        }
        // Allow a trailing number to bind to the word ("hw3", "lab2").
        let base = w.trim_end_matches(|c: char| c.is_ascii_digit());
        base == needle && base.len() != w.len()
    })
}

/// Academic-term regex: a season word plus a 4-digit year, in either order.
fn term_regex() -> &'static regex::Regex {
    static R: OnceLock<regex::Regex> = OnceLock::new();
    R.get_or_init(|| {
        regex::Regex::new(
            r"(?:^|[\s.,_/-])(fall|autumn|spring|summer|winter)[\s.,_/-]*(\d{4})|(\d{4})[\s.,_/-]*(fall|autumn|spring|summer|winter)",
        )
        .expect("valid term regex")
    })
}

/// Extract an academic term ("Fall 2026") from a file name, if present.
pub fn extract_term(file_name: &str) -> Option<Term> {
    let hay = file_name.to_ascii_lowercase();
    let caps = term_regex().captures(&hay)?;
    let (season, year) = if let (Some(s), Some(y)) = (caps.get(1), caps.get(2)) {
        (s.as_str(), y.as_str())
    } else if let (Some(y), Some(s)) = (caps.get(3), caps.get(4)) {
        (s.as_str(), y.as_str())
    } else {
        return None;
    };
    let year: i32 = year.parse().ok()?;
    if !(1990..=2099).contains(&year) {
        return None;
    }
    Some(match season {
        "fall" | "autumn" => Term::Fall(year),
        "spring" => Term::Spring(year),
        "summer" => Term::Summer(year),
        "winter" => Term::Winter(year),
        _ => return None,
    })
}

/// Course-code regex: 2-4 letters then 2-4 digits, optionally separated by a
/// space (e.g. "CS101", "BIO 210", "MATH2210").
fn course_regex() -> &'static regex::Regex {
    static R: OnceLock<regex::Regex> = OnceLock::new();
    R.get_or_init(|| {
        regex::Regex::new(r"(?:^|[\s._/-])([a-z]{2,4})\s?(\d{2,4})(?:[a-z]?)(?:[\s._/-]|$)")
            .expect("valid course regex")
    })
}

/// Pull a normalized course code ("CS101") out of a space-normalized haystack.
fn extract_course_code(haystack: &str) -> Option<String> {
    // Season words followed by a year look like course codes to the loose
    // pattern ("fall 2026"); exclude them so a term isn't misread as a course.
    const SEASONS: &[&str] = &["fall", "autu", "spri", "summ", "wint"];
    for caps in course_regex().captures_iter(haystack) {
        let letters = caps.get(1)?.as_str();
        let digits = caps.get(2)?.as_str();
        if SEASONS.contains(&&letters[..letters.len().min(4)]) {
            continue;
        }
        return Some(format!("{}{}", letters.to_ascii_uppercase(), digits));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_coursework_types() {
        let cases = [
            ("CS101 homework 3.pdf", CourseworkKind::Assignment),
            ("BIO210 Lab report.docx", CourseworkKind::Lab),
            ("Midterm study guide.pdf", CourseworkKind::Exam),
            ("Lecture 12 slides.pptx", CourseworkKind::Lecture),
            ("chapter 5 reading.pdf", CourseworkKind::Reading),
            ("final project proposal.docx", CourseworkKind::Project),
            ("syllabus.pdf", CourseworkKind::Syllabus),
        ];
        for (name, expected) in cases {
            let c = classify_coursework(name).unwrap_or_else(|| panic!("no match for {name}"));
            assert_eq!(c.kind, expected, "file {name}");
        }
    }

    #[test]
    fn extracts_course_code() {
        assert_eq!(
            classify_coursework("CS101 homework 3.pdf").unwrap().course,
            Some("CS101".to_string())
        );
        assert_eq!(
            classify_coursework("BIO 210 lab.docx").unwrap().course,
            Some("BIO210".to_string())
        );
    }

    #[test]
    fn bare_course_code_is_coursework() {
        let c = classify_coursework("MATH2210 handout.pdf").unwrap();
        assert_eq!(c.kind, CourseworkKind::Generic);
        assert_eq!(c.course, Some("MATH2210".to_string()));
    }

    #[test]
    fn non_coursework_returns_none() {
        assert!(classify_coursework("vacation.jpg").is_none());
        assert!(classify_coursework("grocery_list.txt").is_none());
    }

    #[test]
    fn short_tokens_do_not_match_inside_words() {
        // "lab" must not match "label", "hw" must not match "however".
        assert!(classify_coursework("label_template.docx").is_none());
        assert!(classify_coursework("however_notes_are_here.pdf").is_some()); // matches "notes", not "hw"
    }

    #[test]
    fn hw_with_trailing_number_matches_assignment() {
        assert_eq!(
            classify_coursework("hw3.pdf").unwrap().kind,
            CourseworkKind::Assignment
        );
    }

    #[test]
    fn extracts_academic_term_either_order() {
        assert_eq!(
            extract_term("CS101 Fall 2026 notes.pdf"),
            Some(Term::Fall(2026))
        );
        assert_eq!(
            extract_term("2027-Spring-syllabus.pdf"),
            Some(Term::Spring(2027))
        );
        assert_eq!(
            extract_term("summer2025_reading.pdf"),
            Some(Term::Summer(2025))
        );
    }

    #[test]
    fn term_is_not_mistaken_for_course_code() {
        // "Fall 2026" should be a term, not a course code "FALL2026".
        let c = classify_coursework("Fall 2026 lecture.pdf").unwrap();
        assert_eq!(c.course, None);
        assert_eq!(
            extract_term("Fall 2026 lecture.pdf"),
            Some(Term::Fall(2026))
        );
    }

    #[test]
    fn no_term_returns_none() {
        assert_eq!(extract_term("random notes.pdf"), None);
    }
}
