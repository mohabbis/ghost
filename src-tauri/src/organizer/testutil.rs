//! Tiny test-only filesystem helpers, shared across the organizer tests.
//!
//! Kept dependency-free on purpose: the planner must be testable without the UI
//! and without pulling extra crates just for a scratch directory. Compiled only
//! under `#[cfg(test)]`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A unique temp directory removed on drop.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write `bytes` to `rel` under this temp dir (creating parent dirs),
    /// returning its absolute path.
    pub fn file(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.path.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, bytes).unwrap();
        p
    }

    /// Create `rel` (and any parent dirs) under this temp dir, returning its
    /// absolute path.
    pub fn dir(&self, rel: &str) -> PathBuf {
        let p = self.path.join(rel);
        fs::create_dir_all(&p).unwrap();
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Create a fresh, unique temp directory for a test.
pub fn tempdir() -> TempDir {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("ghost_organizer_test_{pid}_{n}"));
    fs::create_dir_all(&path).unwrap();
    TempDir { path }
}
