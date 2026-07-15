//! Deterministic identity-document scanning for the Guard Desk desk check.
//!
//! This module turns the *already OCR'd* text of an identity credential (a
//! driver's license, state ID, or passport) into structured, reviewable fields
//! plus a small set of derived compliance signals (age, expiry state, review
//! flags).
//!
//! It follows the same discipline as the rest of the trusted core:
//!
//! - **Rules first, AI never.** Parsing is a pure function of the input text —
//!   labelled-field regexes and a handful of deterministic heuristics. No
//!   network, no model, no filesystem, no OS input.
//! - **No raw image handling here.** OCR (which does touch image bytes) lives in
//!   [`crate::core::ocr`]; this module only sees text, so it is classified
//!   `safe-read` and is trivially unit-testable off the UI.
//! - **Surfaces uncertainty.** Missing or suspicious fields become human
//!   readable `flags` rather than silent guesses, so the desk operator can
//!   review before anything is cashed.
//!
//! Nothing in here decides an outcome — it only extracts and annotates. The
//! frontend compliance evaluation and the human operator remain in charge.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// The kind of identity document detected from the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdDocumentType {
    DriversLicense,
    StateId,
    Passport,
    Unknown,
}

/// Structured result of scanning an identity document's OCR text.
///
/// Every extracted field is optional: OCR is imperfect and not every document
/// carries every field. Absent fields are reported as `None` (and surfaced in
/// [`IdScan::flags`]) rather than being invented.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdScan {
    /// Cardholder full name, normalized to a single spaced string.
    pub name: Option<String>,
    /// Date of birth as detected (original string form, e.g. `1992-08-14`).
    pub dob: Option<String>,
    /// License / ID number.
    pub id_number: Option<String>,
    /// Expiration date (original string form).
    pub expiry: Option<String>,
    /// Issue date (original string form).
    pub issue_date: Option<String>,
    /// Street/residence address line, when present.
    pub address: Option<String>,
    /// Sex/gender marker (`M`, `F`, or `X`).
    pub sex: Option<String>,
    /// License class (e.g. `C`, `D`, `CDL`).
    pub class: Option<String>,
    /// Issuing US jurisdiction (full state name) when it can be inferred.
    pub jurisdiction: Option<String>,
    /// Detected document type.
    pub document_type: IdDocumentType,
    /// Cardholder age in whole years, computed from `dob` relative to today.
    pub age: Option<u32>,
    /// `true` when the document is past its expiration date.
    pub expired: Option<bool>,
    /// `true` when the document expires within [`EXPIRES_SOON_DAYS`].
    pub expires_soon: Option<bool>,
    /// Human-readable review notes for anything missing or suspicious.
    pub flags: Vec<String>,
}

/// A document expiring within this many days is flagged "expires soon" so the
/// desk operator can warn the customer even though the ID is still valid.
pub const EXPIRES_SOON_DAYS: i64 = 30;

/// Minimum age (years) treated as an adult cardholder. Below this the scan
/// flags the cardholder as a minor.
pub const ADULT_AGE: u32 = 18;

/// Scan an identity document's OCR text using the current system date for the
/// age/expiry derivations. See [`scan_id_at`] for a deterministic variant.
pub fn scan_id(text: &str) -> IdScan {
    scan_id_at(text, today_ymd())
}

/// Scan against an explicit `today` (`(year, month, day)`), so age and expiry
/// derivations are deterministic and unit-testable.
pub fn scan_id_at(text: &str, today: (i32, u32, u32)) -> IdScan {
    let fields = parse_id_fields(text);
    let document_type = detect_document_type(text);

    let age = fields
        .dob
        .as_deref()
        .and_then(parse_date)
        .and_then(|dob| compute_age(dob, today));

    let expiry_ymd = fields.expiry.as_deref().and_then(parse_date);
    let expired = expiry_ymd.map(|e| ymd_cmp(e, today) < 0);
    let expires_soon = expiry_ymd.map(|e| {
        let days = days_between(today, e);
        (0..=EXPIRES_SOON_DAYS).contains(&days)
    });

    let mut flags = Vec::new();
    if fields.name.is_none() {
        flags.push("Cardholder name not detected".to_string());
    }
    if fields.dob.is_none() {
        flags.push("Date of birth not detected".to_string());
    }
    if fields.id_number.is_none() {
        flags.push("ID number not detected".to_string());
    }
    if fields.expiry.is_none() {
        flags.push("Expiration date not detected".to_string());
    }
    if expired == Some(true) {
        flags.push("ID is expired".to_string());
    } else if expires_soon == Some(true) {
        flags.push(format!("ID expires within {EXPIRES_SOON_DAYS} days"));
    }
    if let Some(a) = age
        && a < ADULT_AGE
    {
        flags.push(format!("Cardholder is a minor (age {a})"));
    }
    if document_type == IdDocumentType::Unknown {
        flags.push("Document type not recognized".to_string());
    }

    IdScan {
        name: fields.name,
        dob: fields.dob,
        id_number: fields.id_number,
        expiry: fields.expiry,
        issue_date: fields.issue_date,
        address: fields.address,
        sex: fields.sex,
        class: fields.class,
        jurisdiction: fields.jurisdiction,
        document_type,
        age,
        expired,
        expires_soon,
        flags,
    }
}

/// Raw extracted fields before any derivation. Internal to keep [`IdScan`] the
/// single public shape.
struct IdFields {
    name: Option<String>,
    dob: Option<String>,
    id_number: Option<String>,
    expiry: Option<String>,
    issue_date: Option<String>,
    address: Option<String>,
    sex: Option<String>,
    class: Option<String>,
    jurisdiction: Option<String>,
}

fn parse_id_fields(text: &str) -> IdFields {
    let name = extract_name(text);
    let id_number = first_capture(
        text,
        &[
            r"(?i)\bDLN\b[^\S\n]*[:.#]?[^\S\n]*([A-Za-z0-9-]*\d[A-Za-z0-9-]{2,})",
            r"(?i)\b(?:driver'?s?\s+license|license|lic|ID|DL)[^\S\n]*(?:no\.?|number|#)[^\S\n]*[:.#]?[^\S\n]*([A-Za-z0-9-]*\d[A-Za-z0-9-]*)",
            r"(?i)\bpassport[^\S\n]*(?:no\.?|number|#)?[^\S\n]*[:.#]?[^\S\n]*([A-Za-z0-9]{6,})",
            r"(?i)\b(?:ID|DL)[^\S\n]+([A-Za-z]{0,2}-?\d{5,}[A-Za-z0-9-]*)",
        ],
    );
    let dob = first_date(
        text,
        &[r"(?i)(?:DOB|D\.O\.B\.?|date\s+of\s+birth|birth\s*date|DBB)\s*[:.]?\s*"],
    );
    let expiry = first_date(
        text,
        &[r"(?i)(?:EXP(?:IRES|IRATION)?|EXP\s*DATE|DBA|4b)\s*[:.]?\s*"],
    );
    let issue_date = first_date(
        text,
        &[r"(?i)(?:ISS(?:UED|UE)?|ISSUE\s*DATE|DBD|4a)\s*[:.]?\s*"],
    );
    let address = first_capture(
        text,
        &[r"(?i)(?:address|addr|residence)\s*[:.]?\s*([0-9][A-Za-z0-9 .,#'/-]{3,})"],
    );
    let sex = first_capture(text, &[r"(?i)(?:sex|gender)\s*[:.]?\s*([MFX])\b"])
        .map(|s| s.to_ascii_uppercase());
    let class = first_capture(text, &[r"(?i)(?:class|cls)\s*[:.]?\s*([A-Za-z]{1,3})\b"])
        .map(|s| s.to_ascii_uppercase());
    let jurisdiction = detect_jurisdiction(text, id_number.as_deref());

    IdFields {
        name,
        dob,
        id_number,
        expiry,
        issue_date,
        address,
        sex,
        class,
        jurisdiction,
    }
}

/// Names appear either behind a `NAME:` label or, on AAMVA-style scans, split
/// across `LN`/`FN` (last/first) lines. Try the labelled form first, then
/// reassemble the AAMVA form into `First Last`.
fn extract_name(text: &str) -> Option<String> {
    if let Some(labelled) = first_capture(
        text,
        &[
            r"(?i)full\s+name\s*[:.]?\s*([A-Za-z][A-Za-z .,'-]+)",
            r"(?i)\bname\s*[:.]?\s*([A-Za-z][A-Za-z .,'-]+)",
        ],
    ) {
        return Some(reorder_name(&labelled));
    }

    // AAMVA element codes: DCS/LN = family name, DAC/FN = first name.
    let last = first_capture(
        text,
        &[r"(?i)\b(?:DCS|LN)\b\s*[:.]?\s*([A-Za-z][A-Za-z .'-]+)"],
    );
    let first = first_capture(
        text,
        &[r"(?i)\b(?:DAC|FN)\b\s*[:.]?\s*([A-Za-z][A-Za-z .'-]+)"],
    );
    match (first, last) {
        (Some(f), Some(l)) => Some(normalize_name(&format!("{f} {l}"))),
        (Some(f), None) => Some(normalize_name(&f)),
        (None, Some(l)) => Some(normalize_name(&l)),
        (None, None) => None,
    }
}

fn normalize_name(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Reorder a `LAST, FIRST [MIDDLE]` name into `FIRST [MIDDLE] LAST`. Many IDs
/// (and OCR passes) surface the surname first with a comma. Names without a
/// comma are returned as-is (whitespace-normalized).
fn reorder_name(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches(',').trim();
    if let Some((last, rest)) = trimmed.split_once(',') {
        let last = last.trim();
        let rest = rest.trim();
        if !last.is_empty() && !rest.is_empty() {
            return normalize_name(&format!("{rest} {last}"));
        }
    }
    normalize_name(trimmed)
}

/// Map characters commonly confused by OCR back to the digits they almost
/// certainly are. Only applied to strings that are expected to be numeric
/// (date and, when parsing, number tokens) so letters in real words are never
/// touched.
fn normalize_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'O' | 'o' | 'Q' | 'D' => '0',
            'I' | 'l' | 'L' | 'i' | '|' => '1',
            'S' | 's' => '5',
            'B' => '8',
            'Z' | 'z' => '2',
            'G' => '6',
            other => other,
        })
        .collect()
}

fn detect_document_type(text: &str) -> IdDocumentType {
    let upper = text.to_ascii_uppercase();
    if upper.contains("PASSPORT") {
        IdDocumentType::Passport
    } else if upper.contains("DRIVER") {
        IdDocumentType::DriversLicense
    } else if upper.contains("IDENTIFICATION CARD")
        || upper.contains("IDENTITY CARD")
        || upper.contains("STATE ID")
        || upper.contains("ID CARD")
    {
        IdDocumentType::StateId
    } else {
        IdDocumentType::Unknown
    }
}

/// Infer the issuing US jurisdiction: prefer a two-letter state prefix on the
/// ID number (e.g. `NY-...`), then fall back to a full state name appearing in
/// the text.
fn detect_jurisdiction(text: &str, id_number: Option<&str>) -> Option<String> {
    if let Some(num) = id_number {
        let trimmed = num.trim();
        if trimmed.len() >= 3 {
            let prefix: String = trimmed.chars().take(2).collect();
            let next = trimmed.chars().nth(2);
            let boundary = next.map(|c| !c.is_ascii_alphabetic()).unwrap_or(true);
            if boundary && let Some(name) = state_name(&prefix.to_ascii_uppercase()) {
                return Some(name.to_string());
            }
        }
    }

    let upper = text.to_ascii_uppercase();
    US_STATES
        .iter()
        .find(|(_, name)| upper.contains(&name.to_ascii_uppercase()))
        .map(|(_, name)| name.to_string())
}

fn state_name(code: &str) -> Option<&'static str> {
    US_STATES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| *name)
}

const US_STATES: &[(&str, &str)] = &[
    ("AL", "Alabama"),
    ("AK", "Alaska"),
    ("AZ", "Arizona"),
    ("AR", "Arkansas"),
    ("CA", "California"),
    ("CO", "Colorado"),
    ("CT", "Connecticut"),
    ("DE", "Delaware"),
    ("FL", "Florida"),
    ("GA", "Georgia"),
    ("HI", "Hawaii"),
    ("ID", "Idaho"),
    ("IL", "Illinois"),
    ("IN", "Indiana"),
    ("IA", "Iowa"),
    ("KS", "Kansas"),
    ("KY", "Kentucky"),
    ("LA", "Louisiana"),
    ("ME", "Maine"),
    ("MD", "Maryland"),
    ("MA", "Massachusetts"),
    ("MI", "Michigan"),
    ("MN", "Minnesota"),
    ("MS", "Mississippi"),
    ("MO", "Missouri"),
    ("MT", "Montana"),
    ("NE", "Nebraska"),
    ("NV", "Nevada"),
    ("NH", "New Hampshire"),
    ("NJ", "New Jersey"),
    ("NM", "New Mexico"),
    ("NY", "New York"),
    ("NC", "North Carolina"),
    ("ND", "North Dakota"),
    ("OH", "Ohio"),
    ("OK", "Oklahoma"),
    ("OR", "Oregon"),
    ("PA", "Pennsylvania"),
    ("RI", "Rhode Island"),
    ("SC", "South Carolina"),
    ("SD", "South Dakota"),
    ("TN", "Tennessee"),
    ("TX", "Texas"),
    ("UT", "Utah"),
    ("VT", "Vermont"),
    ("VA", "Virginia"),
    ("WA", "Washington"),
    ("WV", "West Virginia"),
    ("WI", "Wisconsin"),
    ("WY", "Wyoming"),
];

/// Return the first capturing-group-1 match across `patterns`, trimmed.
fn first_capture(text: &str, patterns: &[&str]) -> Option<String> {
    for p in patterns {
        let re = Regex::new(p).ok()?;
        if let Some(caps) = re.captures(text)
            && let Some(m) = caps.get(1)
        {
            let s = m.as_str().trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// Match a labelled date: each prefix pattern is anchored and a date token
/// (ISO or slash) is appended, so `DOB: 1992-08-14` and `DOB 08/14/1992` both
/// work. Returns the raw date string as written on the document.
fn first_date(text: &str, prefixes: &[&str]) -> Option<String> {
    // Digit-or-OCR-confusable class, kept in sync with `normalize_digits`.
    const D: &str = r"[0-9OoIlLSBZzGQ|]";
    let date = format!(r"({D}{{1,4}}[-/. ]{D}{{1,2}}[-/. ]{D}{{2,4}})");
    for prefix in prefixes {
        let pattern = format!("{prefix}{date}");
        if let Some(found) = first_capture(text, &[&pattern]) {
            return Some(normalize_digits(&found));
        }
    }
    None
}

/// Parse a date token into `(year, month, day)`, separator-agnostic (`-`, `/`,
/// `.`, space) and tolerant of OCR digit confusions. Order is inferred from the
/// component widths: a leading four-digit group is ISO (`Y M D`), otherwise the
/// token is read as US (`M D Y`). Two-digit years are windowed like strptime
/// (`00..=68` -> `2000..=2068`, `69..=99` -> `1969..=1999`), which keeps birth
/// years in the past while near-future expiries stay in the future.
fn parse_date(value: &str) -> Option<(i32, u32, u32)> {
    let norm = normalize_digits(value.trim());
    let parts: Vec<&str> = norm
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() != 3 {
        return None;
    }
    let (year_str, month_str, day_str) = if parts[0].len() == 4 {
        (parts[0], parts[1], parts[2])
    } else {
        (parts[2], parts[0], parts[1])
    };
    let mut year: i32 = year_str.parse().ok()?;
    let month: u32 = month_str.parse().ok()?;
    let day: u32 = day_str.parse().ok()?;
    if year_str.len() <= 2 {
        year = match year {
            0..=68 => 2000 + year,
            69..=99 => 1900 + year,
            _ => year,
        };
    }
    valid_ymd(year, month, day)
}

fn valid_ymd(y: i32, m: u32, d: u32) -> Option<(i32, u32, u32)> {
    if (1..=12).contains(&m) && (1..=31).contains(&d) && y > 0 {
        Some((y, m, d))
    } else {
        None
    }
}

/// Compare two `(y, m, d)` dates: negative if `a < b`, zero if equal, positive
/// if `a > b`.
fn ymd_cmp(a: (i32, u32, u32), b: (i32, u32, u32)) -> i32 {
    match a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// Whole-year age of someone born on `dob` as of `today`. Returns `None` for
/// implausible values (future birth date or age > 150).
fn compute_age(dob: (i32, u32, u32), today: (i32, u32, u32)) -> Option<u32> {
    let mut age = today.0 - dob.0;
    if (today.1, today.2) < (dob.1, dob.2) {
        age -= 1;
    }
    if (0..=150).contains(&age) {
        Some(age as u32)
    } else {
        None
    }
}

/// Signed day count from `a` to `b` (positive when `b` is later than `a`).
fn days_between(a: (i32, u32, u32), b: (i32, u32, u32)) -> i64 {
    days_from_civil(b) - days_from_civil(a)
}

/// Days since the Unix epoch for a civil date. Howard Hinnant's algorithm.
fn days_from_civil((y, m, d): (i32, u32, u32)) -> i64 {
    let y = y as i64;
    let m = m as i64;
    let d = d as i64;
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// The reverse of [`days_from_civil`]: civil date for a Unix-epoch day count.
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

fn today_ymd() -> (i32, u32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_from_days(secs.div_euclid(86_400))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: (i32, u32, u32) = (2026, 7, 10);

    #[test]
    fn parses_labelled_drivers_license() {
        let text = "STATE OF CALIFORNIA DRIVER LICENSE\n\
             NAME: JOHN A DOE\n\
             DOB: 1992-08-14\n\
             DL NO: D1234567\n\
             EXP: 2029-12-15\n\
             ISS: 2021-12-15\n\
             SEX: M CLASS: C\n\
             ADDRESS: 123 MAIN ST";
        let scan = scan_id_at(text, TODAY);
        assert_eq!(scan.name.as_deref(), Some("JOHN A DOE"));
        assert_eq!(scan.dob.as_deref(), Some("1992-08-14"));
        assert_eq!(scan.id_number.as_deref(), Some("D1234567"));
        assert_eq!(scan.expiry.as_deref(), Some("2029-12-15"));
        assert_eq!(scan.issue_date.as_deref(), Some("2021-12-15"));
        assert_eq!(scan.sex.as_deref(), Some("M"));
        assert_eq!(scan.class.as_deref(), Some("C"));
        assert_eq!(scan.address.as_deref(), Some("123 MAIN ST"));
        assert_eq!(scan.document_type, IdDocumentType::DriversLicense);
        assert_eq!(scan.age, Some(33));
        assert_eq!(scan.expired, Some(false));
        assert_eq!(scan.expires_soon, Some(false));
        assert!(scan.flags.is_empty(), "clean ID should have no flags");
    }

    #[test]
    fn parses_slash_dates_and_two_digit_years() {
        let text = "IDENTIFICATION CARD\nName: Jane Smith\nDOB 11/23/85\nEXP 05/18/30";
        let scan = scan_id_at(text, TODAY);
        assert_eq!(scan.dob.as_deref(), Some("11/23/85"));
        assert_eq!(scan.age, Some(40));
        assert_eq!(scan.expired, Some(false));
        assert_eq!(scan.document_type, IdDocumentType::StateId);
    }

    #[test]
    fn flags_expired_id() {
        let text = "DRIVER LICENSE\nNAME: BOB LATE\nDOB: 1980-01-01\nEXP: 2026-02-14";
        let scan = scan_id_at(text, TODAY);
        assert_eq!(scan.expired, Some(true));
        assert!(scan.flags.iter().any(|f| f.contains("expired")));
    }

    #[test]
    fn flags_expires_soon() {
        let text = "DRIVER LICENSE\nNAME: SOON EXP\nDOB: 1980-01-01\nEXP: 2026-07-30";
        let scan = scan_id_at(text, TODAY);
        assert_eq!(scan.expired, Some(false));
        assert_eq!(scan.expires_soon, Some(true));
        assert!(scan.flags.iter().any(|f| f.contains("expires within")));
    }

    #[test]
    fn flags_minor_cardholder() {
        let text = "DRIVER LICENSE\nNAME: YOUNG ONE\nDOB: 2012-01-01\nEXP: 2030-01-01";
        let scan = scan_id_at(text, TODAY);
        assert_eq!(scan.age, Some(14));
        assert!(scan.flags.iter().any(|f| f.contains("minor")));
    }

    #[test]
    fn missing_fields_become_flags() {
        let scan = scan_id_at("SOME UNRELATED TEXT", TODAY);
        assert!(scan.name.is_none());
        assert!(scan.dob.is_none());
        assert!(scan.id_number.is_none());
        assert!(scan.expiry.is_none());
        assert_eq!(scan.document_type, IdDocumentType::Unknown);
        assert!(scan.flags.iter().any(|f| f.contains("name not detected")));
        assert!(
            scan.flags
                .iter()
                .any(|f| f.contains("Date of birth not detected"))
        );
        assert!(
            scan.flags
                .iter()
                .any(|f| f.contains("Document type not recognized"))
        );
    }

    #[test]
    fn aamva_last_first_name_reassembled() {
        let text = "DL\nLN DOE\nFN JOHN\nDBB 1992-08-14";
        let scan = scan_id_at(text, TODAY);
        assert_eq!(scan.name.as_deref(), Some("JOHN DOE"));
        assert_eq!(scan.dob.as_deref(), Some("1992-08-14"));
    }

    #[test]
    fn jurisdiction_from_id_number_prefix() {
        let text = "IDENTIFICATION CARD\nNAME: JANE SMITH\nID NO: NY-88219421";
        let scan = scan_id_at(text, TODAY);
        assert_eq!(scan.jurisdiction.as_deref(), Some("New York"));
    }

    #[test]
    fn jurisdiction_from_state_name_in_text() {
        let text = "STATE OF ILLINOIS DRIVER LICENSE\nNAME: JOHN DOE\nDL 12345678";
        let scan = scan_id_at(text, TODAY);
        assert_eq!(scan.jurisdiction.as_deref(), Some("Illinois"));
    }

    #[test]
    fn detects_passport() {
        let text = "PASSPORT\nNAME: TRAVELER ONE\nPASSPORT NO: X1234567\nEXP: 2031-01-01";
        let scan = scan_id_at(text, TODAY);
        assert_eq!(scan.document_type, IdDocumentType::Passport);
        assert_eq!(scan.id_number.as_deref(), Some("X1234567"));
    }

    #[test]
    fn scan_is_deterministic() {
        let text = "DRIVER LICENSE\nNAME: JOHN DOE\nDOB: 1990-01-01\nEXP: 2030-01-01";
        assert_eq!(scan_id_at(text, TODAY), scan_id_at(text, TODAY));
    }

    #[test]
    fn age_boundary_before_and_after_birthday() {
        // Born 2008-07-11: the day before the 18th birthday is still 17.
        assert_eq!(compute_age((2008, 7, 11), (2026, 7, 10)), Some(17));
        // On the birthday, 18.
        assert_eq!(compute_age((2008, 7, 11), (2026, 7, 11)), Some(18));
    }

    #[test]
    fn civil_day_round_trips() {
        for &d in &[(2026, 7, 10), (2000, 1, 1), (1970, 1, 1), (1992, 8, 14)] {
            assert_eq!(civil_from_days(days_from_civil(d)), d);
        }
    }

    #[test]
    fn future_birth_date_has_no_age() {
        assert_eq!(compute_age((2030, 1, 1), TODAY), None);
    }

    #[test]
    fn tolerates_dot_separators_and_ocr_digit_confusions() {
        // `l992` (lowercase L for 1), dot separators, `O1` (letter O for 0).
        let text = "DRIVER LICENSE\nNAME: JOHN DOE\nDOB: l992.O8.14\nEXP 2030.O1.O1";
        let scan = scan_id_at(text, TODAY);
        assert_eq!(scan.dob.as_deref(), Some("1992.08.14"));
        assert_eq!(scan.age, Some(33));
        assert_eq!(scan.expiry.as_deref(), Some("2030.01.01"));
        assert_eq!(scan.expired, Some(false));
    }

    #[test]
    fn tolerates_space_separated_us_date() {
        let text = "IDENTIFICATION CARD\nName: Jane Doe\nEXP 05 18 2030";
        let scan = scan_id_at(text, TODAY);
        assert_eq!(scan.expiry.as_deref(), Some("05 18 2030"));
        assert_eq!(scan.expired, Some(false));
    }

    #[test]
    fn reorders_last_comma_first_name() {
        let text = "DRIVER LICENSE\nNAME: DOE, JOHN A\nDOB: 1990-01-01";
        let scan = scan_id_at(text, TODAY);
        assert_eq!(scan.name.as_deref(), Some("JOHN A DOE"));
    }

    #[test]
    fn parse_date_reads_iso_us_and_dotted_forms() {
        assert_eq!(parse_date("1992-08-14"), Some((1992, 8, 14)));
        assert_eq!(parse_date("08/14/1992"), Some((1992, 8, 14)));
        assert_eq!(parse_date("2030.01.01"), Some((2030, 1, 1)));
        assert_eq!(parse_date("11/23/85"), Some((1985, 11, 23)));
        assert_eq!(parse_date("05 18 30"), Some((2030, 5, 18)));
        // Junk / wrong arity -> no date.
        assert_eq!(parse_date("not a date"), None);
        assert_eq!(parse_date("2026-13-40"), None);
    }
}
