//! Deterministic reporting/academic **period** extraction from a file name.
//!
//! Pure string logic — no IO, no locale, no clock. Given a name like
//! `"Q2 2026 Income Statement.xlsx"` or `"BIO101 Fall2026 midterm.pdf"`, it
//! finds the period the file belongs to so the filer can build a stable,
//! chronological folder hierarchy. Shared by every filing profile
//! ([`super::finance`], [`super::academic`]).
//!
//! Matching is intentionally conservative: patterns require real delimiters so
//! a stray 4-digit number inside a word is not mistaken for a year. When
//! nothing matches, callers file under an "Undated" bucket rather than guessing.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// A calendar period a file can be filed under, from coarse (year) to fine (day).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "granularity")]
pub enum Period {
    Annual { year: i32 },
    Quarter { year: i32, quarter: u8 },
    Month { year: i32, month: u8 },
    Day { year: i32, month: u8, day: u8 },
}

const MONTH_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

impl Period {
    /// The calendar year this period belongs to.
    pub fn year(&self) -> i32 {
        match self {
            Period::Annual { year }
            | Period::Quarter { year, .. }
            | Period::Month { year, .. }
            | Period::Day { year, .. } => *year,
        }
    }

    /// Folder segments for a chronological filing hierarchy, coarsest first,
    /// e.g. `Quarter{2026,2}` -> `["2026", "Q2"]`.
    pub fn dir_segments(&self) -> Vec<String> {
        match *self {
            Period::Annual { year } => vec![format!("{year:04}")],
            Period::Quarter { year, quarter } => vec![format!("{year:04}"), format!("Q{quarter}")],
            Period::Month { year, month } => {
                vec![format!("{year:04}"), format!("{year:04}-{month:02}")]
            }
            Period::Day { year, month, day } => vec![
                format!("{year:04}"),
                format!("{year:04}-{month:02}"),
                format!("{year:04}-{month:02}-{day:02}"),
            ],
        }
    }

    /// A short, human-readable label, e.g. `"Q2 2026"`, `"Jun 2026"`.
    pub fn label(&self) -> String {
        match *self {
            Period::Annual { year } => format!("{year:04}"),
            Period::Quarter { year, quarter } => format!("Q{quarter} {year:04}"),
            Period::Month { year, month } => {
                format!("{} {year:04}", MONTH_SHORT[(month - 1) as usize])
            }
            Period::Day { year, month, day } => {
                format!("{} {day}, {year:04}", MONTH_SHORT[(month - 1) as usize])
            }
        }
    }
}

const MIN_YEAR: i32 = 1990;
const MAX_YEAR: i32 = 2099;

fn valid_year(y: i32) -> bool {
    (MIN_YEAR..=MAX_YEAR).contains(&y)
}

fn month_from_name(token: &str) -> Option<u8> {
    let lower = token.trim().to_ascii_lowercase();
    let head = &lower[..lower.len().min(3)];
    let m = match head {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return None,
    };
    Some(m)
}

// Month-name alternation (full or abbreviated). Longer forms first so the
// engine prefers "march" over "mar".
const MONTHS: &str = r"(jan(?:uary)?|feb(?:ruary)?|mar(?:ch)?|apr(?:il)?|may|jun(?:e)?|jul(?:y)?|aug(?:ust)?|sep(?:t(?:ember)?)?|oct(?:ober)?|nov(?:ember)?|dec(?:ember)?)";

// A leading boundary: start of string or a real separator. Rust's `regex` has
// no look-around, so we match the separator as part of the pattern; we only
// read the captured groups, never the span, so consuming a delimiter is fine.
const PFX: &str = r"(?:^|[\s.,_/-])";

struct Patterns {
    iso_date: regex::Regex,
    us_date: regex::Regex,
    month_day_year: regex::Regex,
    month_year: regex::Regex,
    year_month_name: regex::Regex,
    quarter_first: regex::Regex,
    year_quarter: regex::Regex,
    year_month_num: regex::Regex,
    month_num_year: regex::Regex,
    fiscal_year: regex::Regex,
    bare_year: regex::Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| {
        let re = |s: &str| regex::Regex::new(s).expect("valid period regex");
        Patterns {
            iso_date: re(&format!(
                r"{PFX}(\d{{4}})[-/._](\d{{1,2}})[-/._](\d{{1,2}})"
            )),
            us_date: re(&format!(
                r"{PFX}(\d{{1,2}})[-/._](\d{{1,2}})[-/._](\d{{4}})"
            )),
            month_day_year: re(&format!(
                r"{PFX}{MONTHS}[\s.,_/-]+(\d{{1,2}})(?:st|nd|rd|th)?[\s.,_/-]+(\d{{4}})"
            )),
            month_year: re(&format!(r"{PFX}{MONTHS}[\s.,_/'-]+(\d{{4}})")),
            year_month_name: re(&format!(r"(\d{{4}})[\s.,_/-]+{MONTHS}")),
            quarter_first: re(&format!(r"{PFX}q([1-4])[\s.,_/-]*(\d{{4}})")),
            year_quarter: re(r"(\d{4})[\s.,_/-]*q([1-4])"),
            year_month_num: re(&format!(r"{PFX}(\d{{4}})[-/._](\d{{1,2}})")),
            month_num_year: re(&format!(r"{PFX}(\d{{1,2}})[-/._](\d{{4}})")),
            fiscal_year: re(&format!(r"{PFX}fy[\s.,_/'-]*(\d{{2,4}})")),
            bare_year: re(&format!(r"{PFX}(19\d{{2}}|20\d{{2}})(?:[\s.,_/-]|$)")),
        }
    })
}

fn cap_i32(caps: &regex::Captures, i: usize) -> Option<i32> {
    caps.get(i)?.as_str().parse().ok()
}

/// Extract the calendar period referenced by a file name (or any text).
///
/// Patterns are tried from most specific (full dates) to least (a bare year),
/// so `"2026-06-30"` yields a [`Period::Day`] while `"budget 2026"` yields a
/// [`Period::Annual`]. Returns `None` when no delimited period is present.
pub fn extract_period(text: &str) -> Option<Period> {
    let hay = text.to_ascii_lowercase();
    let p = patterns();

    // 1. ISO date: YYYY-MM-DD.
    for c in p.iso_date.captures_iter(&hay) {
        if let (Some(y), Some(m), Some(d)) = (cap_i32(&c, 1), cap_i32(&c, 2), cap_i32(&c, 3)) {
            if let Some(period) = make_day(y, m, d) {
                return Some(period);
            }
        }
    }

    // 2. US date: MM-DD-YYYY (swap if the first field can only be a day).
    for c in p.us_date.captures_iter(&hay) {
        if let (Some(a), Some(b), Some(y)) = (cap_i32(&c, 1), cap_i32(&c, 2), cap_i32(&c, 3)) {
            let (month, day) = if a > 12 && b <= 12 { (b, a) } else { (a, b) };
            if let Some(period) = make_day(y, month, day) {
                return Some(period);
            }
        }
    }

    // 3. Month name + day + year.
    for c in p.month_day_year.captures_iter(&hay) {
        if let (Some(mon), Some(d), Some(y)) = (
            c.get(1).and_then(|m| month_from_name(m.as_str())),
            cap_i32(&c, 2),
            cap_i32(&c, 3),
        ) {
            if let Some(period) = make_day(y, mon as i32, d) {
                return Some(period);
            }
        }
    }

    // 4. Month name + year (either order).
    for c in p.month_year.captures_iter(&hay) {
        if let (Some(mon), Some(y)) = (
            c.get(1).and_then(|m| month_from_name(m.as_str())),
            cap_i32(&c, 2),
        ) {
            if valid_year(y) {
                return Some(Period::Month {
                    year: y,
                    month: mon,
                });
            }
        }
    }
    for c in p.year_month_name.captures_iter(&hay) {
        if let (Some(y), Some(mon)) = (
            cap_i32(&c, 1),
            c.get(2).and_then(|m| month_from_name(m.as_str())),
        ) {
            if valid_year(y) {
                return Some(Period::Month {
                    year: y,
                    month: mon,
                });
            }
        }
    }

    // 5. Quarter (either order).
    for c in p.quarter_first.captures_iter(&hay) {
        if let (Some(q), Some(y)) = (cap_i32(&c, 1), cap_i32(&c, 2)) {
            if valid_year(y) && (1..=4).contains(&q) {
                return Some(Period::Quarter {
                    year: y,
                    quarter: q as u8,
                });
            }
        }
    }
    for c in p.year_quarter.captures_iter(&hay) {
        if let (Some(y), Some(q)) = (cap_i32(&c, 1), cap_i32(&c, 2)) {
            if valid_year(y) && (1..=4).contains(&q) {
                return Some(Period::Quarter {
                    year: y,
                    quarter: q as u8,
                });
            }
        }
    }

    // 6. Numeric year-month (year-first, then month-first).
    for c in p.year_month_num.captures_iter(&hay) {
        if let (Some(y), Some(m)) = (cap_i32(&c, 1), cap_i32(&c, 2)) {
            if valid_year(y) && (1..=12).contains(&m) {
                return Some(Period::Month {
                    year: y,
                    month: m as u8,
                });
            }
        }
    }
    for c in p.month_num_year.captures_iter(&hay) {
        if let (Some(m), Some(y)) = (cap_i32(&c, 1), cap_i32(&c, 2)) {
            if valid_year(y) && (1..=12).contains(&m) {
                return Some(Period::Month {
                    year: y,
                    month: m as u8,
                });
            }
        }
    }

    // 7. Fiscal year (FY24 / FY2026 / FY'26).
    for c in p.fiscal_year.captures_iter(&hay) {
        if let Some(raw) = c.get(1).map(|m| m.as_str()) {
            if let Some(y) = normalize_year(raw) {
                return Some(Period::Annual { year: y });
            }
        }
    }

    // 8. Bare four-digit year.
    for c in p.bare_year.captures_iter(&hay) {
        if let Some(y) = cap_i32(&c, 1) {
            if valid_year(y) {
                return Some(Period::Annual { year: y });
            }
        }
    }

    None
}

fn make_day(year: i32, month: i32, day: i32) -> Option<Period> {
    if valid_year(year) && (1..=12).contains(&month) && (1..=31).contains(&day) {
        Some(Period::Day {
            year,
            month: month as u8,
            day: day as u8,
        })
    } else {
        None
    }
}

/// Turn a 2- or 4-digit fiscal-year token into a full year (`24` -> `2024`).
fn normalize_year(raw: &str) -> Option<i32> {
    let n: i32 = raw.parse().ok()?;
    let year = if raw.len() <= 2 { 2000 + n } else { n };
    valid_year(year).then_some(year)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso_dates_as_day() {
        assert_eq!(
            extract_period("Bank Statement 2026-06-30.pdf"),
            Some(Period::Day {
                year: 2026,
                month: 6,
                day: 30
            })
        );
    }

    #[test]
    fn parses_us_dates_and_swaps_when_needed() {
        // MM-DD-YYYY
        assert_eq!(
            extract_period("recon_06-15-2026.xlsx"),
            Some(Period::Day {
                year: 2026,
                month: 6,
                day: 15
            })
        );
        // 25 can only be a day -> swap to DD-MM-YYYY reading.
        assert_eq!(
            extract_period("recon_25-06-2026.xlsx"),
            Some(Period::Day {
                year: 2026,
                month: 6,
                day: 25
            })
        );
    }

    #[test]
    fn parses_month_name_plus_year() {
        assert_eq!(
            extract_period("June 2026 P&L.xlsx"),
            Some(Period::Month {
                year: 2026,
                month: 6
            })
        );
        assert_eq!(
            extract_period("2026 December close.xlsx"),
            Some(Period::Month {
                year: 2026,
                month: 12
            })
        );
        // underscore separator (the classic \b pitfall)
        assert_eq!(
            extract_period("march_2026_report.csv"),
            Some(Period::Month {
                year: 2026,
                month: 3
            })
        );
    }

    #[test]
    fn parses_month_day_year_with_ordinal_and_comma() {
        assert_eq!(
            extract_period("Statement June 30, 2026.pdf"),
            Some(Period::Day {
                year: 2026,
                month: 6,
                day: 30
            })
        );
    }

    #[test]
    fn parses_quarters_either_order() {
        assert_eq!(
            extract_period("Q2 2026 Income Statement.xlsx"),
            Some(Period::Quarter {
                year: 2026,
                quarter: 2
            })
        );
        assert_eq!(
            extract_period("income_2026-Q3.xlsx"),
            Some(Period::Quarter {
                year: 2026,
                quarter: 3
            })
        );
        assert_eq!(
            extract_period("2026q4.csv"),
            Some(Period::Quarter {
                year: 2026,
                quarter: 4
            })
        );
    }

    #[test]
    fn parses_numeric_year_month() {
        assert_eq!(
            extract_period("trial_balance_2026-06.csv"),
            Some(Period::Month {
                year: 2026,
                month: 6
            })
        );
        assert_eq!(
            extract_period("06-2026 payroll.xlsx"),
            Some(Period::Month {
                year: 2026,
                month: 6
            })
        );
    }

    #[test]
    fn parses_fiscal_year() {
        assert_eq!(
            extract_period("Budget FY2027.xlsx"),
            Some(Period::Annual { year: 2027 })
        );
        assert_eq!(
            extract_period("forecast_fy24.xlsx"),
            Some(Period::Annual { year: 2024 })
        );
    }

    #[test]
    fn parses_bare_year_as_annual() {
        assert_eq!(
            extract_period("Operating Budget 2026.xlsx"),
            Some(Period::Annual { year: 2026 })
        );
    }

    #[test]
    fn does_not_invent_periods() {
        assert_eq!(extract_period("summary.xlsx"), None);
        assert_eq!(extract_period("invoice_4471.pdf"), None); // 4471 not a year
        assert_eq!(extract_period("form_941.pdf"), None); // tax form, not a period
        assert_eq!(extract_period("top 3 goals.txt"), None);
    }

    #[test]
    fn month_substring_in_word_is_not_matched() {
        // "mar" inside "summary" must not read as March.
        assert_eq!(
            extract_period("summary 2026.xlsx"),
            Some(Period::Annual { year: 2026 })
        );
        // "may" inside "mayor" must not read as May.
        assert_eq!(extract_period("mayor_budget.xlsx"), None);
    }

    #[test]
    fn out_of_range_values_are_rejected() {
        assert_eq!(
            extract_period("2026-13.csv"),
            Some(Period::Annual { year: 2026 })
        ); // month 13 invalid -> falls back to year
        assert_eq!(extract_period("1899 ledger.xlsx"), None); // year below range
    }

    #[test]
    fn dir_segments_and_labels() {
        assert_eq!(
            Period::Quarter {
                year: 2026,
                quarter: 2
            }
            .dir_segments(),
            vec!["2026".to_string(), "Q2".to_string()]
        );
        assert_eq!(
            Period::Month {
                year: 2026,
                month: 6
            }
            .label(),
            "Jun 2026"
        );
        assert_eq!(Period::Annual { year: 2026 }.label(), "2026");
    }

    #[test]
    fn extraction_is_deterministic() {
        assert_eq!(
            extract_period("Q2 2026 report.xlsx"),
            extract_period("Q2 2026 report.xlsx")
        );
    }
}
