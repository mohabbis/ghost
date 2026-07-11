//! Redact common PII patterns from audit text before it leaves the app.
//!
//! The Organizer's filing preset routes tax/financial documents (`docs/filing-profiles.md`),
//! so a file's own name can embed an SSN, card number, email, or phone number
//! (e.g. `138-45-9821_w2.pdf`). Those names flow verbatim into `Capability`
//! paths and can end up in exported audit CSV/JSON/compliance reports. This
//! module only touches the text produced for those exports — the stored
//! [`super::audit_log::AuditLog`] used for the tamper-evident hash chain and
//! undo is untouched, so masking here cannot desync a run's seal.

use std::sync::LazyLock;

use regex::Regex;

// No trailing `\b`: filenames commonly butt a PII-shaped run of digits right
// up against a `_` or extension dot (`138-45-9821_w2.pdf`), and `_`/digits
// are both "word" characters, so a trailing `\b` would silently fail to
// match there. The exact digit-group/separator shape is distinctive enough
// on its own without it.
static SSN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}").unwrap());

static CARD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{4}[ -]\d{4}[ -]\d{4}[ -]\d{1,4}").unwrap());

static EMAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[\w.+-]+@[\w-]+\.[\w.-]+\b").unwrap());

static PHONE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\(?\d{3}\)?[ .-]\d{3}[ .-]\d{4}").unwrap());

/// Replace SSN-, card-, email-, and phone-shaped substrings with a
/// `[REDACTED:kind]` marker. Conservative on purpose: it only matches
/// clearly-delimited patterns (dashes/dots/spaces as separators), so it won't
/// touch ordinary filenames, dates, or plain long numbers.
pub fn mask(text: &str) -> String {
    let text = SSN.replace_all(text, "[REDACTED:ssn]");
    let text = CARD.replace_all(&text, "[REDACTED:card]");
    let text = EMAIL.replace_all(&text, "[REDACTED:email]");
    let text = PHONE.replace_all(&text, "[REDACTED:phone]");
    text.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_ssn_in_a_filename() {
        assert_eq!(
            mask("/z/Documents/138-45-9821_w2.pdf"),
            "/z/Documents/[REDACTED:ssn]_w2.pdf"
        );
    }

    #[test]
    fn masks_card_number() {
        assert_eq!(
            mask("statement 4111-1111-1111-1111.pdf"),
            "statement [REDACTED:card].pdf"
        );
    }

    #[test]
    fn masks_email() {
        assert_eq!(
            mask("invoice from jane.doe@example.com"),
            "invoice from [REDACTED:email]"
        );
    }

    #[test]
    fn masks_phone_number() {
        assert_eq!(mask("call 555-123-4567 back"), "call [REDACTED:phone] back");
    }

    #[test]
    fn leaves_ordinary_filenames_alone() {
        assert_eq!(
            mask("/z/Documents/report_2024.pdf"),
            "/z/Documents/report_2024.pdf"
        );
        assert_eq!(mask("IMG_20240115_103000.jpg"), "IMG_20240115_103000.jpg");
    }
}
