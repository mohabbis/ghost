//! Versioned SQLite migrations, driven by `PRAGMA user_version`.
//!
//! Ghost's live storage backend is redb (see `storage::mod`), not SQLite. This
//! module's only remaining job is bringing a **legacy** pre-redb `ghost.db`
//! SQLite file up to its final schema before `storage::sqlite_import` reads it
//! for the one-time migration into redb — so an install that hasn't been
//! opened since schema V2 still gets a predictable, fully-migrated row shape
//! to import from.
//!
//! `migrate` applies every migration newer than the database's current
//! `user_version`, in order, then records the new version. It is idempotent:
//! re-running against an up-to-date database is a no-op.

use rusqlite::Connection;

/// The schema version this binary produces.
pub const LATEST_VERSION: i64 = 6;

/// Bring `conn` up to [`LATEST_VERSION`], applying forward migrations only.
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    // SQLite leaves foreign keys off per-connection by default; enable them so
    // `ON DELETE CASCADE` on folder rules actually fires.
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    let mut version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if version < 1 {
        conn.execute_batch(MIGRATION_V1)?;
        version = 1;
        // user_version does not accept bound parameters; `version` is an
        // internal constant, never user input, so formatting it is safe.
        conn.execute_batch(&format!("PRAGMA user_version = {version};"))?;
    }

    if version < 2 {
        conn.execute_batch(MIGRATION_V2)?;
        version = 2;
        conn.execute_batch(&format!("PRAGMA user_version = {version};"))?;
    }

    if version < 3 {
        conn.execute_batch(MIGRATION_V3)?;
        version = 3;
        conn.execute_batch(&format!("PRAGMA user_version = {version};"))?;
    }

    if version < 4 {
        conn.execute_batch(MIGRATION_V4)?;
        version = 4;
        conn.execute_batch(&format!("PRAGMA user_version = {version};"))?;
    }

    if version < 5 {
        conn.execute_batch(MIGRATION_V5)?;
        version = 5;
        conn.execute_batch(&format!("PRAGMA user_version = {version};"))?;
    }

    if version < 6 {
        conn.execute_batch(MIGRATION_V6)?;
        version = 6;
        conn.execute_batch(&format!("PRAGMA user_version = {version};"))?;
    }

    Ok(())
}

/// v0 -> v1: create the Zone and folder-rule tables.
const MIGRATION_V1: &str = r#"
CREATE TABLE zones (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  default_decision TEXT NOT NULL CHECK (default_decision IN ('deny', 'ask', 'allow')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE zone_folder_rules (
  id TEXT PRIMARY KEY,
  zone_id TEXT NOT NULL REFERENCES zones(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  can_read INTEGER NOT NULL DEFAULT 1,
  can_create INTEGER NOT NULL DEFAULT 0,
  can_rename INTEGER NOT NULL DEFAULT 0,
  can_move INTEGER NOT NULL DEFAULT 0,
  can_copy INTEGER NOT NULL DEFAULT 0,
  can_delete INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_zone_folder_rules_zone ON zone_folder_rules(zone_id);
"#;

/// v1 -> v2: record executed Organizer runs so a past run can be reviewed and
/// undone. The audit log and undo journal are stored verbatim as JSON (both are
/// `serde`-serializable), keeping all disk-mutation logic in `crate::organizer`
/// while this table is a neutral, append-only history.
const MIGRATION_V2: &str = r#"
CREATE TABLE organizer_executions (
  id TEXT PRIMARY KEY,
  zone_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  applied INTEGER NOT NULL,
  skipped INTEGER NOT NULL,
  failed INTEGER NOT NULL,
  audit_json TEXT NOT NULL,
  undo_json TEXT NOT NULL
);

CREATE INDEX idx_organizer_executions_zone ON organizer_executions(zone_id);
"#;

/// v2 -> v3: opt-in dated renaming for bookkeeping-style filing zones.
const MIGRATION_V3: &str = r#"
ALTER TABLE zones ADD COLUMN rename_dated INTEGER NOT NULL DEFAULT 0;
"#;

/// v3 -> v4: per-rule trust levels (automate / ask_first / never) and the
/// local-only milestone table used to measure time-to-first-value. Existing
/// rules default to `ask_first` — exactly the behavior they had before the
/// column existed.
const MIGRATION_V4: &str = r#"
ALTER TABLE zone_folder_rules ADD COLUMN trust_level TEXT NOT NULL DEFAULT 'ask_first'
  CHECK (trust_level IN ('automate', 'ask_first', 'never'));

CREATE TABLE organizer_milestones (
  key TEXT PRIMARY KEY,
  at TEXT NOT NULL
);
"#;

/// v4 -> v5: tamper-evidence for stored Organizer executions. Each run is
/// sealed with a SHA-256 `hash` over its own row bytes plus the previous run's
/// hash, forming a chain that can be verified offline. Existing rows keep an
/// empty hash (unsealed, pre-upgrade); the verifiable chain begins at the first
/// execution written after this migration.
const MIGRATION_V5: &str = r#"
ALTER TABLE organizer_executions ADD COLUMN hash TEXT NOT NULL DEFAULT '';
ALTER TABLE organizer_executions ADD COLUMN prev_hash TEXT NOT NULL DEFAULT '';
"#;

/// v5 -> v6: write-ahead durability for executions. `organizer_execute` now
/// inserts a row *before* touching the filesystem and updates it after every
/// action, rather than holding the whole run in memory and writing it once at
/// the end — so a crash mid-run leaves a row Ghost can recognize and offer to
/// undo on next launch, instead of losing the undo journal for whatever had
/// already been applied. `finished = 0` marks a row not yet known to have
/// reached a normal end (see `storage::executions::find_unfinished_execution`).
/// Existing rows predate this and are, by definition, already finished.
const MIGRATION_V6: &str = r#"
ALTER TABLE organizer_executions ADD COLUMN finished INTEGER NOT NULL DEFAULT 1;
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_sets_version_and_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, LATEST_VERSION);

        // Running again must not error and must not change the version.
        migrate(&conn).unwrap();
        let v2: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v2, LATEST_VERSION);

        // Tables exist and are empty.
        let zones: i64 = conn
            .query_row("SELECT count(*) FROM zones", [], |r| r.get(0))
            .unwrap();
        assert_eq!(zones, 0);
    }

    #[test]
    fn migration_v3_adds_rename_dated_default_off() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_V1).unwrap();
        conn.execute_batch(MIGRATION_V2).unwrap();
        conn.execute_batch("PRAGMA user_version = 2;").unwrap();
        conn.execute(
            "INSERT INTO zones (id, name, description, default_decision, created_at, updated_at) \
             VALUES ('z', 'Existing', NULL, 'ask', '1', '1')",
            [],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let rename_dated: i64 = conn
            .query_row("SELECT rename_dated FROM zones WHERE id = 'z'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(rename_dated, 0);
    }

    /// Upgrading a v3 database must give existing rules `ask_first` trust —
    /// the exact behavior they had before the column existed.
    #[test]
    fn migration_v4_defaults_existing_rules_to_ask_first() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_V1).unwrap();
        conn.execute_batch(MIGRATION_V2).unwrap();
        conn.execute_batch(MIGRATION_V3).unwrap();
        conn.execute_batch("PRAGMA user_version = 3;").unwrap();
        conn.execute(
            "INSERT INTO zones (id, name, description, default_decision, created_at, updated_at) \
             VALUES ('z', 'Existing', NULL, 'ask', '1', '1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO zone_folder_rules (id, zone_id, path, can_read, can_move) \
             VALUES ('r', 'z', '/home/u/Downloads', 1, 1)",
            [],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let trust: String = conn
            .query_row(
                "SELECT trust_level FROM zone_folder_rules WHERE id = 'r'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(trust, "ask_first");

        // The milestone table exists and starts empty.
        let milestones: i64 = conn
            .query_row("SELECT count(*) FROM organizer_milestones", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(milestones, 0);
    }

    /// Upgrading a v4 database must add `hash`/`prev_hash` to existing
    /// executions with empty defaults — they are "unsealed" pre-upgrade rows,
    /// and the verifiable chain starts at the first execution written after.
    #[test]
    fn migration_v5_adds_empty_hash_columns_to_existing_executions() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_V1).unwrap();
        conn.execute_batch(MIGRATION_V2).unwrap();
        conn.execute_batch(MIGRATION_V3).unwrap();
        conn.execute_batch(MIGRATION_V4).unwrap();
        conn.execute_batch("PRAGMA user_version = 4;").unwrap();
        conn.execute(
            "INSERT INTO organizer_executions \
             (id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json) \
             VALUES ('e', 'z', '1', 0, 0, 0, '[]', '[]')",
            [],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let (hash, prev): (String, String) = conn
            .query_row(
                "SELECT hash, prev_hash FROM organizer_executions WHERE id = 'e'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(hash, "");
        assert_eq!(prev, "");
    }

    /// Upgrading a v5 database must mark existing executions `finished = 1` —
    /// they completed (successfully or not) before this column existed, so
    /// none of them should be mistaken for an interrupted run.
    #[test]
    fn migration_v6_marks_existing_executions_finished() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_V1).unwrap();
        conn.execute_batch(MIGRATION_V2).unwrap();
        conn.execute_batch(MIGRATION_V3).unwrap();
        conn.execute_batch(MIGRATION_V4).unwrap();
        conn.execute_batch(MIGRATION_V5).unwrap();
        conn.execute_batch("PRAGMA user_version = 5;").unwrap();
        conn.execute(
            "INSERT INTO organizer_executions \
             (id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json, hash, prev_hash) \
             VALUES ('e', 'z', '1', 0, 0, 0, '[]', '[]', '', '')",
            [],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let finished: i64 = conn
            .query_row(
                "SELECT finished FROM organizer_executions WHERE id = 'e'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(finished, 1);
    }
}
