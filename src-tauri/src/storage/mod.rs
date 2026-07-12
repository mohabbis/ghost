//! Local redb persistence for trust boundaries (Zones + folder rules),
//! Organizer execution history, and first-touch milestones.
//!
//! The database lives next to the workflow JSON files, at
//! `<data_dir>/ghost/ghost.redb`. Existing installs that still have the
//! pre-redb SQLite database (`<data_dir>/ghost/ghost.db`) are migrated once,
//! automatically, the first time [`open_default`] runs after upgrading — see
//! [`sqlite_import`]. The original SQLite file is renamed, never deleted, as a
//! permanent safety net. Domain types come from `crate::policy`.

use redb::Database;
use std::path::PathBuf;

pub mod executions;
pub mod migrations;
pub mod milestones;
pub mod sqlite_import;
pub mod zones;

/// A handle to the Ghost redb database. Wraps `redb::Database`; when created
/// via [`open_in_memory`] it also owns a temp file that is removed on drop
/// (redb has no first-class in-memory backend in the version pinned here).
pub struct Db {
    pub(crate) db: Database,
    pub(crate) temp_path: Option<PathBuf>,
}

impl Drop for Db {
    fn drop(&mut self) {
        if let Some(path) = &self.temp_path {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Resolve the redb database path, creating the parent directory.
pub fn default_db_path() -> anyhow::Result<PathBuf> {
    Ok(data_dir()?.join("ghost.redb"))
}

/// The pre-redb SQLite database path, kept around only so [`open_default`] can
/// find and import it once on first launch after upgrading.
pub fn legacy_sqlite_db_path() -> anyhow::Result<PathBuf> {
    Ok(data_dir()?.join("ghost.db"))
}

fn data_dir() -> anyhow::Result<PathBuf> {
    let dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?
        .join("ghost");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Open (creating if needed) every table used by the app, inside one write
/// transaction. redb creates a table the first time it is opened in a write
/// transaction, even without inserting anything — calling this right after
/// `Database::create`/import guarantees later read transactions never hit
/// "table does not exist".
fn init_tables(database: &Database) -> anyhow::Result<()> {
    let write_txn = database.begin_write()?;
    zones::init(&write_txn)?;
    milestones::init(&write_txn)?;
    executions::init(&write_txn)?;
    write_txn.commit()?;
    Ok(())
}

/// Open (creating if needed) the Ghost database. If this is the first launch
/// after upgrading from the SQLite-backed storage and a legacy database is
/// still present, its contents are imported once before returning.
pub fn open_default() -> anyhow::Result<Db> {
    let path = default_db_path()?;
    if !path.exists() {
        let legacy_path = legacy_sqlite_db_path()?;
        if legacy_path.exists() {
            sqlite_import::migrate(&legacy_path, &path)?;
        }
    }
    let db = Database::create(&path)?;
    init_tables(&db)?;
    Ok(Db {
        db,
        temp_path: None,
    })
}

/// Open a fresh, isolated database for tests. Backed by a real temp file; it
/// is removed when the returned `Db` is dropped.
///
/// Not `#[cfg(test)]`-gated: `tests/canonical_workflows.rs` is a separate
/// integration-test crate that only sees this crate's normal public API, so
/// this needs to be a real (if test-labeled) part of it — the same reason
/// that test reimplements its own temp-dir helper instead of reusing the
/// `#[cfg(test)]`-gated `organizer::testutil`.
pub fn open_in_memory() -> anyhow::Result<Db> {
    let path = std::env::temp_dir().join(format!("ghost-test-{}.redb", uuid::Uuid::new_v4()));
    let db = Database::create(&path)?;
    init_tables(&db)?;
    Ok(Db {
        db,
        temp_path: Some(path),
    })
}
