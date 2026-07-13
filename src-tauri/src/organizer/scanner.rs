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
/// permission error does not sink an otherwise valid plan. Directories are
/// read in parallel (via `jwalk`) since deep trees are IO-bound; the result is
/// still sorted before it reaches the planner, so the parallelism is
/// invisible to callers — same files in, same order out, every time.
pub fn scan(root: &Path) -> Vec<ScannedFile> {
    // jwalk numbers the root itself at depth 0 and files directly inside it at
    // depth 1, one more than the `scan_into`-style depth this bound used to
    // gate on. `min_depth(1)` drops the root entry (a directory, and not a
    // `ScannedFile` candidate anyway); `max_depth` is shifted by one to match.
    let mut out: Vec<ScannedFile> = jwalk::WalkDir::new(root)
        .skip_hidden(false) // `is_ignored` already covers dotfiles/dirs
        .min_depth(1)
        .max_depth(MAX_DEPTH + 1)
        .process_read_dir(|_depth, _dir, _state, children| {
            children.retain(|entry| match entry {
                Ok(e) => !is_ignored(&e.file_name().to_string_lossy()),
                Err(_) => false, // unreadable entry: skip, don't fail the scan
            });
        })
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        // Symlinks and other entry kinds are intentionally excluded: the MVP
        // organizes plain files only.
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            describe(&entry.path(), &name)
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
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

    /// Symlinks are excluded via jwalk's default no-follow behavior (a
    /// symlink entry reports `is_symlink()==true`/`is_file()==false`, so the
    /// `is_file()` filter in `scan` drops it) — the MVP organizes plain files
    /// only. Windows symlink creation needs `SeCreateSymbolicLinkPrivilege`
    /// (admin/Developer Mode), which the `windows-latest` CI runner may
    /// lack, so this is verified on unix only; the exclusion mechanism
    /// itself is platform-agnostic, but stays unverified by CI on Windows —
    /// an explicit, known residual gap rather than a faked pass there.
    #[cfg(unix)]
    #[test]
    fn scan_excludes_symlinks() {
        let tmp = tempdir();
        let real = tmp.file("report.pdf", b"x");
        tmp.symlink_file("shortcut.pdf", &real);

        let files = scan(tmp.path());
        let names: Vec<&str> = files.iter().map(|f| f.file_name.as_str()).collect();
        assert_eq!(names, vec!["report.pdf"]);
    }

    #[test]
    fn scan_reads_unicode_filenames() {
        let tmp = tempdir();
        tmp.file("日本語 📷 café.pdf", b"x");

        let files = scan(tmp.path());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name, "日本語 📷 café.pdf");
        assert_eq!(files[0].stem, "日本語 📷 café");
        assert_eq!(files[0].extension.as_deref(), Some("pdf"));
    }

    /// A file at `MAX_DEPTH` levels of nesting is included; one level deeper
    /// is not. Pins the exact boundary so a future depth-accounting change
    /// (e.g. adjusting jwalk's depth offset) can't silently shift it.
    #[test]
    fn respects_max_depth_boundary() {
        let tmp = tempdir();
        let mut rel = String::new();
        for i in 1..=(MAX_DEPTH + 2) {
            rel.push_str(&format!("d{i}/"));
            tmp.file(&format!("{rel}f{i}.txt"), b"x");
        }
        let files = scan(tmp.path());
        let depths: Vec<usize> = files
            .iter()
            .map(|f| {
                f.path
                    .strip_prefix(tmp.path())
                    .unwrap()
                    .components()
                    .count()
            })
            .collect();
        assert!(depths.contains(&(MAX_DEPTH + 1)));
        assert!(!depths.contains(&(MAX_DEPTH + 2)));
    }
}
