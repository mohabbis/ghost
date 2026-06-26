//! Versioned SQLite migrations, driven by `PRAGMA user_version`.
//!
//! `migrate` applies every migration newer than the database's current
//! `user_version`, in order, then records the new version. It is idempotent:
//! re-running against an up-to-date database is a no-op.

use rusqlite::Connection;

/// The schema version this binary produces.
pub const LATEST_VERSION: i64 = 2;

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
}
