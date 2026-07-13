//! Stable file identity for TOCTOU detection between scan and execute.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Platform-neutral identity captured at scan time and re-checked at execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileIdentity {
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dev: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ino: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_index: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_serial: Option<u64>,
}

impl FileIdentity {
    /// Read identity from a regular file path. Returns `None` for symlinks or
    /// unreadable metadata.
    pub fn from_path(path: &Path) -> Option<Self> {
        if path.is_symlink() {
            return None;
        }
        let meta = std::fs::metadata(path).ok()?;
        if !meta.is_file() {
            return None;
        }
        Some(Self::from_metadata(&meta))
    }

    pub fn from_metadata(meta: &std::fs::Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            FileIdentity {
                size: meta.len(),
                dev: Some(meta.dev()),
                ino: Some(meta.ino()),
                file_index: None,
                volume_serial: None,
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            // `file_index` / `volume_serial_number` need unstable `windows_by_handle`.
            // Use stable NTFS timestamps as the Windows identity tuple instead.
            FileIdentity {
                size: meta.len(),
                dev: None,
                ino: None,
                file_index: Some(meta.last_write_time()),
                volume_serial: Some(meta.creation_time()),
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            FileIdentity {
                size: meta.len(),
                dev: None,
                ino: None,
                file_index: None,
                volume_serial: None,
            }
        }
    }

    /// True when both sides captured the same platform-specific identifiers.
    pub fn matches(&self, other: &Self) -> bool {
        if self.size != other.size {
            return false;
        }
        #[cfg(unix)]
        {
            self.dev == other.dev && self.ino == other.ino
        }
        #[cfg(windows)]
        {
            self.file_index == other.file_index && self.volume_serial == other.volume_serial
        }
        #[cfg(not(any(unix, windows)))]
        {
            self.size == other.size
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::testutil::tempdir;

    #[test]
    fn same_file_matches_itself() {
        let tmp = tempdir();
        let path = tmp.file("doc.pdf", b"hello");
        let id1 = FileIdentity::from_path(&path).expect("identity");
        let id2 = FileIdentity::from_path(&path).expect("identity");
        assert!(id1.matches(&id2));
    }

    #[test]
    #[cfg(unix)]
    fn different_inodes_do_not_match() {
        let a = FileIdentity {
            size: 5,
            dev: Some(1),
            ino: Some(10),
            file_index: None,
            volume_serial: None,
        };
        let b = FileIdentity {
            size: 5,
            dev: Some(1),
            ino: Some(11),
            file_index: None,
            volume_serial: None,
        };
        assert!(!a.matches(&b));
    }

    #[test]
    #[cfg(windows)]
    fn different_timestamps_do_not_match() {
        let a = FileIdentity {
            size: 5,
            dev: None,
            ino: None,
            file_index: Some(10),
            volume_serial: Some(20),
        };
        let b = FileIdentity {
            size: 5,
            dev: None,
            ino: None,
            file_index: Some(11),
            volume_serial: Some(20),
        };
        assert!(!a.matches(&b));
    }
}
