//! Safe target filename proposal.
//!
//! Pure string logic, no IO. Two jobs:
//!
//! 1. [`safe_file_name`] — sanitize a name so it is valid cross-platform
//!    (Windows is the strict one) without changing meaning.
//! 2. [`deduplicate`] — when a target name is already taken, derive a distinct
//!    ` (2)`, ` (3)`, … variant. The Organizer **never silently overwrites**
//!    (`AGENTS.md` non-negotiable rule), so collisions are resolved by renaming,
//!    never by clobbering.

use std::collections::HashSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Characters that are illegal in filenames on Windows (a superset of the
/// POSIX restrictions, so sanitizing for Windows is safe everywhere).
const ILLEGAL: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Sanitize a single filename component.
///
/// Replaces illegal and control characters with `_`, collapses runs of
/// whitespace, and trims leading/trailing spaces and dots (trailing dots are
/// invalid on Windows). Never returns an empty string — falls back to `file`.
/// The extension, if any, is preserved and lowercased-as-is is *not* applied
/// (we keep the original case of the name).
pub fn safe_file_name(name: &str) -> String {
    let (stem, ext) = split_extension(name);

    let cleaned_stem = sanitize_component(stem);
    let stem_final = if cleaned_stem.is_empty() {
        "file".to_string()
    } else {
        cleaned_stem
    };

    match ext {
        Some(ext) => {
            let cleaned_ext = sanitize_component(ext);
            if cleaned_ext.is_empty() {
                stem_final
            } else {
                format!("{stem_final}.{cleaned_ext}")
            }
        }
        None => stem_final,
    }
}

/// Prefix `name` with the file timestamp's `YYYY-MM` bucket.
///
/// Idempotent for names already beginning with `YYYY-MM ` or `YYYY-MM-`, so
/// repeated planning never stacks date prefixes. A pre-epoch timestamp
/// (clock damage) returns the name unchanged — better no filing period than
/// an invented "1970-01".
pub fn dated_prefix(name: &str, timestamp: SystemTime) -> String {
    if starts_with_date_prefix(name) {
        return name.to_string();
    }

    let Ok(since_epoch) = timestamp.duration_since(UNIX_EPOCH) else {
        return name.to_string();
    };
    let days = (since_epoch.as_secs() / 86_400) as i64;
    let (year, month, _) = civil_from_days(days);
    format!("{year:04}-{month:02} {name}")
}

fn starts_with_date_prefix(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 8
        && b[0..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && (b[7] == b' ' || b[7] == b'-')
}

/// Convert days since 1970-01-01 to a Gregorian date.
///
/// Adapted from Howard Hinnant's public-domain civil calendar algorithm; kept
/// tiny to avoid a date dependency for one filename prefix.
fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += if m <= 2 { 1 } else { 0 };
    (y as i32, m as u32, d as u32)
}

/// Replace illegal/control chars with `_`, collapse whitespace, trim spaces and
/// dots from the ends.
fn sanitize_component(s: &str) -> String {
    let replaced: String = s
        .chars()
        .map(|c| {
            if ILLEGAL.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();

    // Collapse internal whitespace runs to a single space.
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
        .trim_matches(|c: char| c == '.' || c == ' ')
        .to_string()
}

/// Split `name` into `(stem, Some(ext))`, or `(name, None)` when there is no
/// usable extension. A leading dot (dotfile) is treated as part of the stem,
/// not as an extension separator.
fn split_extension(name: &str) -> (&str, Option<&str>) {
    let path = Path::new(name);
    match (path.file_stem(), path.extension()) {
        (Some(stem), Some(ext)) => (
            stem.to_str().unwrap_or(name),
            Some(ext.to_str().unwrap_or("")),
        ),
        _ => (name, None),
    }
}

/// Return a name not present in `taken`, appending ` (2)`, ` (3)`, … before the
/// extension if needed. Does not insert the chosen name into `taken`; the
/// caller owns that bookkeeping.
pub fn deduplicate(name: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(name) {
        return name.to_string();
    }
    let (stem, ext) = split_extension(name);
    for n in 2..=u32::MAX {
        let candidate = match ext {
            Some(ext) => format!("{stem} ({n}).{ext}"),
            None => format!("{stem} ({n})"),
        };
        if !taken.contains(&candidate) {
            return candidate;
        }
    }
    // Unreachable in practice (would require billions of collisions).
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_illegal_characters() {
        assert_eq!(safe_file_name("a-b:c?.txt"), "a-b_c_.txt");
        assert_eq!(safe_file_name("re:port.PDF"), "re_port.PDF"); // case preserved
    }

    #[test]
    fn collapses_whitespace_and_trims() {
        assert_eq!(safe_file_name("  my   file  .pdf"), "my file.pdf");
    }

    #[test]
    fn illegal_chars_become_underscores_and_empty_falls_back() {
        // A name of all-illegal chars sanitizes to underscores (still a valid,
        // distinct name); only a genuinely empty result falls back to `file`.
        assert_eq!(safe_file_name("???"), "___");
        assert_eq!(safe_file_name(""), "file");
        assert_eq!(safe_file_name("   "), "file"); // whitespace-only collapses away
    }

    #[test]
    fn no_extension_is_handled() {
        assert_eq!(safe_file_name("README"), "README");
    }

    #[test]
    fn unicode_characters_are_preserved_in_safe_file_name() {
        // sanitize_component only touches the ASCII-scoped ILLEGAL set and
        // is_control(); Unicode text (emoji, combining diacritics, CJK) must
        // survive untouched, never stripped or transliterated.
        assert_eq!(safe_file_name("日本語 📷 café.pdf"), "日本語 📷 café.pdf");
    }

    #[test]
    fn deduplicate_is_case_sensitive_by_design() {
        // deduplicate compares names via an exact HashSet<String>, so
        // "Report.pdf" and "report.pdf" are distinct here — correct on the
        // case-sensitive filesystem this test runs on (Linux CI). On the
        // case-*insensitive* filesystems macOS/Windows default to, this
        // exact-match comparison is the root cause of a real collision risk
        // it can't see: a known residual gap Linux CI cannot exercise.
        let mut taken = HashSet::new();
        taken.insert("report.pdf".to_string());
        assert_eq!(deduplicate("Report.pdf", &taken), "Report.pdf");
    }

    #[test]
    fn deduplicate_appends_counter_before_extension() {
        let mut taken = HashSet::new();
        taken.insert("a.pdf".to_string());
        assert_eq!(deduplicate("a.pdf", &taken), "a (2).pdf");
        taken.insert("a (2).pdf".to_string());
        assert_eq!(deduplicate("a.pdf", &taken), "a (3).pdf");
    }

    #[test]
    fn deduplicate_leaves_unique_names_untouched() {
        let taken = HashSet::new();
        assert_eq!(deduplicate("unique.txt", &taken), "unique.txt");
    }

    #[test]
    fn dated_prefix_uses_year_and_month() {
        let timestamp = UNIX_EPOCH + std::time::Duration::from_secs(1_708_300_800);
        assert_eq!(
            dated_prefix("acme-invoice.pdf", timestamp),
            "2024-02 acme-invoice.pdf"
        );
    }

    #[test]
    fn dated_prefix_is_idempotent() {
        let timestamp = UNIX_EPOCH + std::time::Duration::from_secs(1_708_300_800);
        assert_eq!(
            dated_prefix("2024-02 acme-invoice.pdf", timestamp),
            "2024-02 acme-invoice.pdf"
        );
        assert_eq!(
            dated_prefix("2024-02-acme-invoice.pdf", timestamp),
            "2024-02-acme-invoice.pdf"
        );
    }

    #[test]
    fn dated_prefix_runs_before_sanitizing() {
        let timestamp = UNIX_EPOCH + std::time::Duration::from_secs(1_708_300_800);
        assert_eq!(
            safe_file_name(&dated_prefix("acme:invoice?.pdf", timestamp)),
            "2024-02 acme_invoice_.pdf"
        );
    }

    #[test]
    fn dated_prefix_refuses_to_invent_a_period_for_pre_epoch_timestamps() {
        // Clock damage produces pre-epoch mtimes; stamping those "1970-01"
        // would file the document under a period it never belonged to.
        let timestamp = UNIX_EPOCH - std::time::Duration::from_secs(86_400);
        assert_eq!(dated_prefix("a.pdf", timestamp), "a.pdf");
    }
}
