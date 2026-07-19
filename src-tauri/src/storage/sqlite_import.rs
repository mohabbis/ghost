//! One-time import of the legacy SQLite-backed database into redb.
//!
//! Ghost's local storage moved from SQLite to redb (pure-Rust, no bundled C
//! dependency). Existing installs still have their Zones, folder rules,
//! execution/audit history, and tamper-evident hash chain in the old
//! `ghost.db` SQLite file — losing any of that on upgrade is unacceptable, so
//! this module copies every row across, verbatim, the first time
//! `storage::open_default` runs after upgrading.
//!
//! The legacy file is never deleted: on success it is renamed to
//! `<name>.db.migrated` as a permanent safety net.

use crate::storage::{executions, migrations, milestones, zones};
use redb::{Database, WriteTransaction};
use rusqlite::Connection as SqliteConnection;
use std::path::Path;

/// Import `legacy_path` (a SQLite database) into a freshly created redb
/// database at `new_path`, then rename the legacy file so it is never
/// mistaken for still-current storage. No-ops (and does not create
/// `new_path`) if `legacy_path` does not exist.
pub fn migrate(legacy_path: &Path, new_path: &Path) -> anyhow::Result<()> {
    if !legacy_path.exists() {
        return Ok(());
    }

    // Build the new database at a temporary path and only move it into place
    // after the import has fully committed. `open_default` treats the mere
    // presence of `new_path` as "migration already done", so creating redb
    // directly at `new_path` would let a crash mid-import leave an empty
    // database there that permanently blocks re-migration and strands the
    // legacy data — the one outcome this module exists to prevent. A stale
    // temp file from a previously-interrupted attempt is discarded first.
    let importing_path = new_path.with_extension("db.importing");
    let _ = std::fs::remove_file(&importing_path);

    let sqlite = SqliteConnection::open(legacy_path)?;
    // Bring any old install up to the last SQLite schema so field shapes are
    // predictable, regardless of how long ago the user last opened Ghost.
    migrations::migrate(&sqlite)?;

    {
        let redb = Database::create(&importing_path)?;
        let write_txn = redb.begin_write()?;
        zones::init(&write_txn)?;
        milestones::init(&write_txn)?;
        executions::init(&write_txn)?;

        import_zones(&sqlite, &write_txn)?;
        import_milestones(&sqlite, &write_txn)?;
        import_executions(&sqlite, &write_txn)?;

        write_txn.commit()?;
    } // drop `redb` so its file handle is released before the rename below
    // (Windows cannot rename a file that still has an open handle).

    // Close the legacy connection before touching the file it points at.
    // SQLite holds an OS-level lock on the database file for as long as the
    // connection is open; on Windows (unlike POSIX) a locked file cannot be
    // renamed, so skipping this made the rename below silently fail on every
    // real Windows upgrade.
    drop(sqlite);

    // Atomically publish the fully-committed database. `new_path` comes into
    // existence only here, so an import interrupted above always re-runs
    // cleanly from the still-present legacy file on the next launch.
    std::fs::rename(&importing_path, new_path)?;

    let backup_path = legacy_path.with_extension("db.migrated");
    // Best-effort: if this rename fails (e.g. permissions), the redb database
    // is already fully populated and usable — leaving the old file in place
    // is a harmless, if untidy, outcome, never a data-loss one.
    let _ = std::fs::rename(legacy_path, backup_path);

    Ok(())
}

fn import_zones(sqlite: &SqliteConnection, write_txn: &WriteTransaction) -> anyhow::Result<()> {
    let mut stmt = sqlite
        .prepare("SELECT id, name, description, default_decision, rename_dated FROM zones")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)? != 0,
        ))
    })?;
    for row in rows {
        let (id, name, description, decision, rename_dated) = row?;
        zones::import_zone_raw(
            write_txn,
            &id,
            &name,
            description.as_deref(),
            &decision,
            rename_dated,
        )?;
    }
    drop(stmt);

    let mut stmt = sqlite.prepare(
        "SELECT id, zone_id, path, can_read, can_create, can_rename, can_move, can_copy, \
         can_delete, trust_level FROM zone_folder_rules",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)? != 0,
            row.get::<_, i64>(4)? != 0,
            row.get::<_, i64>(5)? != 0,
            row.get::<_, i64>(6)? != 0,
            row.get::<_, i64>(7)? != 0,
            row.get::<_, i64>(8)? != 0,
            row.get::<_, String>(9)?,
        ))
    })?;
    for row in rows {
        let (
            id,
            zone_id,
            path,
            can_read,
            can_create,
            can_rename,
            can_move,
            can_copy,
            can_delete,
            trust_level,
        ) = row?;
        zones::import_folder_rule_raw(
            write_txn,
            &id,
            &zone_id,
            &path,
            can_read,
            can_create,
            can_rename,
            can_move,
            can_copy,
            can_delete,
            &trust_level,
        )?;
    }
    Ok(())
}

fn import_milestones(
    sqlite: &SqliteConnection,
    write_txn: &WriteTransaction,
) -> anyhow::Result<()> {
    let mut stmt = sqlite.prepare("SELECT key, at FROM organizer_milestones")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (key, at) = row?;
        milestones::import_milestone_raw(write_txn, &key, &at)?;
    }
    Ok(())
}

fn import_executions(
    sqlite: &SqliteConnection,
    write_txn: &WriteTransaction,
) -> anyhow::Result<()> {
    // Ascending rowid = original insertion order, which `import_execution_raw`
    // relies on to reproduce the same `seq`/hash-chain order in redb.
    let mut stmt = sqlite.prepare(
        "SELECT id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json, \
         hash, prev_hash, finished FROM organizer_executions ORDER BY rowid ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)? as usize,
            row.get::<_, i64>(4)? as usize,
            row.get::<_, i64>(5)? as usize,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, String>(9)?,
            row.get::<_, i64>(10)? != 0,
        ))
    })?;
    for row in rows {
        let (
            id,
            zone_id,
            created_at,
            applied,
            skipped,
            failed,
            audit_json,
            undo_json,
            hash,
            prev_hash,
            finished,
        ) = row?;
        executions::import_execution_raw(
            write_txn,
            &id,
            &zone_id,
            &created_at,
            applied,
            skipped,
            failed,
            &audit_json,
            &undo_json,
            &hash,
            &prev_hash,
            finished,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Db;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn scratch_paths() -> (PathBuf, PathBuf) {
        let base =
            std::env::temp_dir().join(format!("ghost-sqlite-import-test-{}", Uuid::new_v4()));
        (base.with_extension("db"), base.with_extension("redb"))
    }

    fn open_result(new_path: &Path) -> Db {
        let db = Database::open(new_path).unwrap();
        Db {
            db,
            temp_path: None,
        }
    }

    #[test]
    fn migrates_zones_rules_milestones_and_executions_preserving_order() {
        let (legacy, new_path) = scratch_paths();
        {
            let conn = SqliteConnection::open(&legacy).unwrap();
            migrations::migrate(&conn).unwrap();
            conn.execute(
                "INSERT INTO zones (id, name, description, default_decision, rename_dated, \
                 created_at, updated_at) VALUES ('z1', 'School', NULL, 'ask', 0, '1', '1')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO zone_folder_rules (id, zone_id, path, can_read, can_create, \
                 can_rename, can_move, can_copy, can_delete, trust_level) \
                 VALUES ('r1', 'z1', '/tmp/x', 1, 1, 0, 1, 0, 0, 'ask_first')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO organizer_milestones (key, at) VALUES ('first_plan', '123')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO organizer_executions \
                 (id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json, \
                  hash, prev_hash, finished) \
                 VALUES ('e1', 'z1', '100', 1, 0, 0, '[]', '[]', 'h1', '', 1)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO organizer_executions \
                 (id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json, \
                  hash, prev_hash, finished) \
                 VALUES ('e2', 'z1', '200', 1, 0, 0, '[]', '[]', 'h2', 'h1', 1)",
                [],
            )
            .unwrap();
        }

        migrate(&legacy, &new_path).unwrap();

        assert!(!legacy.exists(), "legacy file should be renamed away");
        assert!(legacy.with_extension("db.migrated").exists());

        let db = open_result(&new_path);

        let zone = zones::get_zone(&db, "z1").unwrap().unwrap();
        assert_eq!(zone.name, "School");

        let rules = zones::list_folder_rules(&db, "z1").unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].path, PathBuf::from("/tmp/x"));

        let ms = milestones::list_milestones(&db).unwrap();
        assert_eq!(ms, vec![("first_plan".to_string(), "123".to_string())]);

        let list = executions::list_executions(&db).unwrap();
        assert_eq!(list.len(), 2);

        // Insertion order (and thus the chain) must be preserved: e1 before
        // e2, verifiable via get_execution's prev_hash linkage.
        let e2 = executions::get_execution(&db, "e2").unwrap().unwrap();
        assert_eq!(e2.prev_hash, "h1");

        drop(db);
        let _ = std::fs::remove_file(&new_path);
        let _ = std::fs::remove_file(legacy.with_extension("db.migrated"));
    }

    #[test]
    fn no_legacy_file_is_a_noop() {
        let (legacy, new_path) = scratch_paths();
        migrate(&legacy, &new_path).unwrap();
        assert!(!new_path.exists());
    }

    #[test]
    fn migration_publishes_atomically_and_cleans_a_stale_temp() {
        let (legacy, new_path) = scratch_paths();
        {
            let conn = SqliteConnection::open(&legacy).unwrap();
            migrations::migrate(&conn).unwrap();
            conn.execute(
                "INSERT INTO zones (id, name, description, default_decision, rename_dated, \
                 created_at, updated_at) VALUES ('z1', 'School', NULL, 'ask', 0, '1', '1')",
                [],
            )
            .unwrap();
        }

        // Leftover partial from a previously-interrupted migration attempt: it
        // must be discarded, not opened or left to collide with the fresh build.
        let importing = new_path.with_extension("db.importing");
        std::fs::write(&importing, b"garbage from a crashed prior attempt").unwrap();

        migrate(&legacy, &new_path).unwrap();

        // The destination exists only after a committed import, and no temp
        // file is left behind.
        assert!(new_path.exists(), "committed db must be published");
        assert!(!importing.exists(), "temp import file must be cleaned up");
        assert!(legacy.with_extension("db.migrated").exists());

        let db = open_result(&new_path);
        assert_eq!(zones::get_zone(&db, "z1").unwrap().unwrap().name, "School");

        drop(db);
        let _ = std::fs::remove_file(&new_path);
        let _ = std::fs::remove_file(legacy.with_extension("db.migrated"));
    }
}
