//! Read-only filesystem scanning for the Organizer planner.
//!
//! The scanner walks a source folder, skips system/ignored entries, and
//! returns lightweight [`ScannedFile`] metadata. It never opens file contents
//! and never mutates anything — it only reads directory entries and `stat`
//! metadata, which keeps it cheap and privacy-respecting.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The maximum directory depth the scanner descends into. Bounded so a deeply
/// nested tree cannot make planning unbounded; deeper files are simply not
/// surfaced (and so are never proposed for a move).
pub const MAX_DEPTH: usize = 8;

/// Metadata for a single file discovered by the scanner. Deliberately small:
/// path, name parts, size, and timestamps — enough for deterministic
/// classification without reading file contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScannedFile {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// File name including extension, e.g. `report.pdf`.
    pub file_name: String,
    /// File name without the final extension, e.g. `report`.
    pub stem: String,
    /// Lowercased extension without the dot, e.g. `pdf`. `None` when absent.
    pub extension: Option<String>,
    /// Size in bytes.
    pub size: u64,
    /// Last-modified time, when the platform reports it.
    pub modified: Option<SystemTime>,
    /// Creation time, when the platform reports it.
    pub created: Option<SystemTime>,
}

/// Should this entry be skipped regardless of whether it is a file or folder?
///
/// Skips dotfiles/dot-directories and a small set of well-known OS bookkeeping
/// and temp/partial-download files. Keeping this deterministic matters: the
/// same tree must always yield the same plan.
pub fn is_ignored(name: &str) -> bool {
    // Hidden entries (covers `.git`, `.DS_Store`, `.localized`, etc.).
    if name.starts_with('.') {
        return true;
    }

    // Known OS/system metadata files.
    const SYSTEM_FILES: &[&str] = &[
        "Thumbs.db",
        "desktop.ini",
        "$RECYCLE.BIN",
        "System Volume Information",
    ];
    if SYSTEM_FILES.iter().any(|s| s.eq_ignore_ascii_case(name)) {
        return true;
    }

    // Temp / lock / partial-download artifacts: not real user content.
    const TEMP_SUFFIXES: &[&str] = &[
        "~",
        ".tmp",
        ".temp",
        ".swp",
        ".part",
        ".partial",
        ".crdownload",
        ".download",
        ".lock",
    ];
    let lower = name.to_ascii_lowercase();
    TEMP_SUFFIXES.iter().any(|s| lower.ends_with(s))
}

/// Scan `root` recursively (up to [`MAX_DEPTH`]) and return the files worth
/// organizing, sorted by path for deterministic output.
///
/// Read-only: descends directories and reads metadata, never contents. Entries
/// it cannot read are skipped rather than aborting the whole scan, so one
/// permission error does not sink an otherwise valid plan.
pub fn scan(root: &Path) -> Vec<ScannedFile> {
    let mut out = Vec::new();
    scan_into(root, 0, &mut out);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn scan_into(dir: &Path, depth: usize, out: &mut Vec<ScannedFile>) {
    if depth > MAX_DEPTH {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return, // unreadable directory: skip, don't fail the scan
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if is_ignored(&name) {
            continue;
        }
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            scan_into(&path, depth + 1, out);
        } else if file_type.is_file() {
            if let Some(scanned) = describe(&path, &name) {
                out.push(scanned);
            }
        }
        // Symlinks and other entry kinds are intentionally ignored: the MVP
        // organizes plain files only.
    }
}

/// Build a [`ScannedFile`] from a path and its file name. Returns `None` if
/// metadata cannot be read.
fn describe(path: &Path, file_name: &str) -> Option<ScannedFile> {
    let meta = std::fs::metadata(path).ok()?;
    let stem = Path::new(file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| file_name.to_string());
    let extension = Path::new(file_name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    Some(ScannedFile {
        path: path.to_path_buf(),
        file_name: file_name.to_string(),
        stem,
        extension,
        size: meta.len(),
        modified: meta.modified().ok(),
        created: meta.created().ok(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::testutil::tempdir;

    #[test]
    fn ignores_system_and_temp_files() {
        assert!(is_ignored(".DS_Store"));
        assert!(is_ignored(".git"));
        assert!(is_ignored("Thumbs.db"));
        assert!(is_ignored("thumbs.db")); // case-insensitive
        assert!(is_ignored("movie.mp4.part"));
        assert!(is_ignored("backup~"));
        assert!(!is_ignored("report.pdf"));
        assert!(!is_ignored("photo.JPG"));
    }

    #[test]
    fn scan_filters_ignored_and_descends_dirs() {
        let tmp = tempdir();
        tmp.file("a.pdf", b"x");
        tmp.file(".DS_Store", b"x");
        tmp.file("draft.tmp", b"x");
        tmp.file("sub/b.png", b"x");

        let files = scan(tmp.path());
        let names: Vec<&str> = files.iter().map(|f| f.file_name.as_str()).collect();
        assert_eq!(names, vec!["a.pdf", "b.png"]); // sorted, ignored excluded
        assert_eq!(files[1].extension.as_deref(), Some("png"));
    }

    #[test]
    fn extension_is_lowercased() {
        let tmp = tempdir();
        tmp.file("IMG.JPEG", b"x");
        let files = scan(tmp.path());
        assert_eq!(files[0].extension.as_deref(), Some("jpeg"));
        assert_eq!(files[0].stem, "IMG");
    }
}
