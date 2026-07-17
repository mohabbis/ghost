//! Route deletes through the OS Trash / Recycle Bin.
//!
//! Ghost's reversibility guarantee has historically lived entirely in its own
//! undo journal. For deletions that is not enough: if the journal is ever lost,
//! corrupted, or buggy, a deleted file would be gone for good. Sending deletes
//! to the OS trash gives a second, independent safety net — the Finder Trash on
//! macOS, the Recycle Bin on Windows, and the freedesktop trash on Linux — so
//! "nothing is deleted silently, nothing is unrecoverable" is backed by the OS,
//! not only by us.
//!
//! This module is the seam. [`Trasher`] is the single operation the executor
//! needs (send an already-vetted path to the OS trash); [`OsTrasher`] is the
//! real implementation, and the in-memory test double lets executor tests assert
//! behavior deterministically without touching a real system trash that CI
//! cannot observe. Deletion is **not yet wired into the runtime executor** — the
//! policy gate (`can_delete`) and this seam are the safe-by-construction
//! foundation so that when delete ships, it routes through the OS trash from day
//! one rather than being retrofitted afterward.

use std::path::Path;

/// Send a single existing path to the OS trash.
///
/// Deliberately narrow: the executor is responsible for re-checking existence,
/// symlink status, and policy before calling this — the trasher's only job is
/// the platform call itself.
pub trait Trasher {
    /// Move `path` to the OS trash. Returns a short, human-readable reference to
    /// where it went (recorded on the undo/audit entry), or an error string.
    fn send_to_trash(&self, path: &Path) -> Result<String, String>;
}

/// The real implementation, backed by the `trash` crate: the freedesktop trash
/// on Linux, the Finder Trash on macOS, and the Recycle Bin on Windows.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsTrasher;

impl Trasher for OsTrasher {
    fn send_to_trash(&self, path: &Path) -> Result<String, String> {
        trash::delete(path).map_err(|e| e.to_string())?;
        Ok(format!("OS Trash/Recycle Bin: {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::testutil::tempdir;
    use std::cell::RefCell;
    use std::path::PathBuf;

    /// A test double that "trashes" by moving files into a holding directory it
    /// owns, so tests can assert the source is gone without depending on a real
    /// system trash CI cannot observe.
    struct RecordingTrasher {
        holding: PathBuf,
        trashed: RefCell<Vec<PathBuf>>,
    }

    impl RecordingTrasher {
        fn new(holding: PathBuf) -> Self {
            std::fs::create_dir_all(&holding).ok();
            Self {
                holding,
                trashed: RefCell::new(Vec::new()),
            }
        }
        fn trashed_paths(&self) -> Vec<PathBuf> {
            self.trashed.borrow().clone()
        }
    }

    impl Trasher for RecordingTrasher {
        fn send_to_trash(&self, path: &Path) -> Result<String, String> {
            let name = path.file_name().ok_or_else(|| "no file name".to_string())?;
            let dest = self.holding.join(name);
            std::fs::rename(path, &dest).map_err(|e| e.to_string())?;
            self.trashed.borrow_mut().push(path.to_path_buf());
            Ok(format!("test-holding:{}", dest.display()))
        }
    }

    #[test]
    fn recording_trasher_removes_source_and_records_it() {
        let tmp = tempdir();
        let f = tmp.file("junk.tmp", b"x");
        let t = RecordingTrasher::new(tmp.path().join(".ghost-holding"));
        let reference = t.send_to_trash(&f).expect("trash ok");
        assert!(!f.exists(), "source must be gone after trashing");
        assert!(reference.starts_with("test-holding:"));
        assert_eq!(t.trashed_paths(), vec![f]);
    }

    #[test]
    fn os_trasher_sends_a_real_file_to_the_system_trash() {
        // Exercises the real OsTrasher end to end on whatever OS the tests run
        // on. We only assert the source is gone (recovery lives in the OS trash);
        // we don't assert on system-trash internals, which vary by platform.
        let tmp = tempdir();
        let f = tmp.file("ghost-os-trasher-probe.tmp", b"x");
        match OsTrasher.send_to_trash(&f) {
            Ok(_) => assert!(!f.exists(), "source must be gone after OS trashing"),
            // Headless CI containers may lack a usable trash location; don't fail
            // the suite over the environment, only over wrong behavior.
            Err(_) => assert!(f.exists(), "on trash failure the file must be untouched"),
        }
    }
}
