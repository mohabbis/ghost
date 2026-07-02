//! Deterministic file classification.
//!
//! Classification is **rules first, AI never** (for this phase): a pure function
//! of the [`ScannedFile`] metadata — extension, then filename and size signals.
//! No network, no model, no IO. Each result carries a `confidence` in `0.0..=1.0`
//! and a human-readable `reason` so the approval UI can show *why* a file was
//! placed where it was, and flag low-confidence items for review.

use super::scanner::ScannedFile;
use serde::{Deserialize, Serialize};

/// A destination category a file can be sorted into. The string form is also
/// the destination subfolder name (see [`Category::folder_name`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Documents,
    Invoices,
    Receipts,
    Statements,
    Spreadsheets,
    Presentations,
    Images,
    Audio,
    Video,
    Archives,
    Code,
    Data,
    Ebooks,
    Fonts,
    Installers,
    /// Fallback when no rule matches; always low confidence.
    Other,
}

impl Category {
    /// The destination subfolder name for this category.
    pub fn folder_name(self) -> &'static str {
        match self {
            Category::Documents => "Documents",
            Category::Invoices => "Invoices",
            Category::Receipts => "Receipts",
            Category::Statements => "Statements",
            Category::Spreadsheets => "Spreadsheets",
            Category::Presentations => "Presentations",
            Category::Images => "Images",
            Category::Audio => "Audio",
            Category::Video => "Video",
            Category::Archives => "Archives",
            Category::Code => "Code",
            Category::Data => "Data",
            Category::Ebooks => "Ebooks",
            Category::Fonts => "Fonts",
            Category::Installers => "Installers",
            Category::Other => "Other",
        }
    }
}

/// The outcome of classifying one file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Classification {
    pub category: Category,
    /// Confidence in `0.0..=1.0`. Below [`LOW_CONFIDENCE`] the planner flags the
    /// item for manual review.
    pub confidence: f32,
    /// Why this category was chosen — surfaced to the user.
    pub reason: String,
}

/// Files at or below this confidence are surfaced as "needs review".
pub const LOW_CONFIDENCE: f32 = 0.5;

/// Confidence assigned to a clean, unambiguous extension match.
const EXT_CONFIDENCE: f32 = 0.95;

/// Confidence assigned when a document-like file is sorted by a filename
/// keyword rather than by extension alone.
const DOCUMENT_KEYWORD_CONFIDENCE: f32 = 0.85;

/// Map a lowercased extension to its category, or `None` if unknown.
fn category_for_extension(ext: &str) -> Option<Category> {
    let cat = match ext {
        // Documents
        "pdf" | "doc" | "docx" | "rtf" | "odt" | "txt" | "md" | "pages" | "tex" => {
            Category::Documents
        }
        // Spreadsheets
        "xls" | "xlsx" | "ods" | "csv" | "tsv" | "numbers" => Category::Spreadsheets,
        // Presentations
        "ppt" | "pptx" | "odp" | "key" => Category::Presentations,
        // Images
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "tif" | "heic" | "webp" | "svg"
        | "raw" | "cr2" | "nef" => Category::Images,
        // Audio
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "aiff" | "wma" => Category::Audio,
        // Video
        "mp4" | "mov" | "avi" | "mkv" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg" => {
            Category::Video
        }
        // Archives
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" => Category::Archives,
        // Code / source
        "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "c" | "h" | "cpp" | "hpp" | "java" | "go"
        | "rb" | "php" | "swift" | "kt" | "sh" | "html" | "css" | "sql" => Category::Code,
        // Structured data / config
        "json" | "xml" | "yaml" | "yml" | "toml" | "ini" | "log" => Category::Data,
        // Ebooks
        "epub" | "mobi" | "azw" | "azw3" => Category::Ebooks,
        // Fonts
        "ttf" | "otf" | "woff" | "woff2" => Category::Fonts,
        // Installers / disk images
        "dmg" | "pkg" | "exe" | "msi" | "deb" | "rpm" | "appimage" | "iso" => Category::Installers,
        _ => return None,
    };
    Some(cat)
}

/// A coarse filename keyword signal. Does not override the extension-derived
/// category (a `.pdf` invoice is still a Document) but enriches the reason so
/// the user sees the planner noticed an intent signal.
fn filename_note(stem_lower: &str) -> Option<&'static str> {
    const KEYWORDS: &[(&str, &str)] = &[
        ("invoice", "looks like an invoice"),
        ("receipt", "looks like a receipt"),
        ("resume", "looks like a resume/CV"),
        ("cv", "looks like a resume/CV"),
        ("statement", "looks like a statement"),
        ("screenshot", "looks like a screenshot"),
        ("scan", "looks like a scan"),
    ];
    KEYWORDS
        .iter()
        .find(|(kw, _)| stem_lower.contains(kw))
        .map(|(_, note)| *note)
}

/// A filename keyword that is strong enough to pick a filing category for
/// document-like files.
fn document_keyword_category(stem_lower: &str) -> Option<Category> {
    if stem_lower.contains("invoice") {
        Some(Category::Invoices)
    } else if stem_lower.contains("receipt") {
        Some(Category::Receipts)
    } else if stem_lower.contains("statement") {
        Some(Category::Statements)
    } else {
        None
    }
}

/// Classify a scanned file deterministically.
///
/// Order of signals: extension (primary), then a filename keyword note. An
/// unknown or absent extension yields [`Category::Other`] with low confidence
/// so the planner can flag it for review rather than guessing.
pub fn classify(file: &ScannedFile) -> Classification {
    let stem_lower = file.stem.to_ascii_lowercase();
    let note = filename_note(&stem_lower);

    match file.extension.as_deref().and_then(category_for_extension) {
        Some(category) => {
            let ext = file.extension.as_deref().unwrap_or("");
            let (category, confidence) = if category == Category::Documents {
                match document_keyword_category(&stem_lower) {
                    Some(keyword_category) => (keyword_category, DOCUMENT_KEYWORD_CONFIDENCE),
                    None => (category, EXT_CONFIDENCE),
                }
            } else {
                (category, EXT_CONFIDENCE)
            };
            let reason = match note {
                Some(n) => format!("`.{ext}` -> {} ({n})", category.folder_name()),
                None => format!("`.{ext}` -> {}", category.folder_name()),
            };
            Classification {
                category,
                confidence,
                reason,
            }
        }
        None => {
            let reason = match &file.extension {
                Some(ext) => format!("Unrecognized extension `.{ext}`; needs review"),
                None => "No file extension; needs review".to_string(),
            };
            Classification {
                category: Category::Other,
                confidence: 0.2,
                reason,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn file(name: &str) -> ScannedFile {
        let p = PathBuf::from(name);
        ScannedFile {
            file_name: name.to_string(),
            stem: p
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            extension: p
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase()),
            path: p,
            size: 1234,
            modified: None,
            created: None,
        }
    }

    #[test]
    fn known_extensions_classify_with_high_confidence() {
        assert_eq!(classify(&file("a.pdf")).category, Category::Documents);
        assert_eq!(classify(&file("b.PNG")).category, Category::Images);
        assert_eq!(classify(&file("c.mp3")).category, Category::Audio);
        assert_eq!(classify(&file("d.zip")).category, Category::Archives);
        assert!(classify(&file("a.pdf")).confidence > LOW_CONFIDENCE);
    }

    #[test]
    fn unknown_extension_is_other_and_low_confidence() {
        let c = classify(&file("mystery.qzx"));
        assert_eq!(c.category, Category::Other);
        assert!(c.confidence <= LOW_CONFIDENCE);
    }

    #[test]
    fn missing_extension_is_low_confidence() {
        let c = classify(&file("README"));
        assert_eq!(c.category, Category::Other);
        assert!(c.confidence <= LOW_CONFIDENCE);
    }

    #[test]
    fn filename_keyword_enriches_reason_without_changing_category() {
        let c = classify(&file("resume.pdf"));
        assert_eq!(c.category, Category::Documents);
        assert!(c.reason.contains("resume"));
    }

    #[test]
    fn document_keywords_choose_filing_categories() {
        let invoice = classify(&file("invoice_acme.pdf"));
        assert_eq!(invoice.category, Category::Invoices);
        assert_eq!(invoice.confidence, DOCUMENT_KEYWORD_CONFIDENCE);
        assert!(invoice.reason.contains("invoice"));

        let statement = classify(&file("bank-statement-2026.pdf"));
        assert_eq!(statement.category, Category::Statements);
        assert_eq!(statement.confidence, DOCUMENT_KEYWORD_CONFIDENCE);

        let receipt = classify(&file("travel_receipt.txt"));
        assert_eq!(receipt.category, Category::Receipts);
        assert_eq!(receipt.confidence, DOCUMENT_KEYWORD_CONFIDENCE);
    }

    #[test]
    fn classification_is_deterministic() {
        let f = file("report.docx");
        assert_eq!(classify(&f), classify(&f));
    }
}
