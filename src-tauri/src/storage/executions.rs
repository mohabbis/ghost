//! SQLite-backed history of executed Organizer runs.
//!
//! When the executor applies an approved plan it produces an [`ExecutionReport`]
//! carrying the [`AuditLog`] (what happened) and the [`UndoJournal`] (how to
//! reverse it). Persisting both lets the UI list past runs and undo one later,
//! even across app restarts. The journal and audit log are stored verbatim as
//! JSON columns — both are already `serde`-serializable — so this module adds no
//! new domain logic, only durable bookkeeping.

use crate::audit::{AuditLog, UndoJournal};
use crate::organizer::executor::ExecutionReport;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Epoch-seconds timestamp as a string, matching `zones::now_ts` bookkeeping.
fn now_ts() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

/// A lightweight row for the history list — counts without the heavy JSON blobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub id: String,
    pub zone_id: String,
    pub created_at: String,
    pub applied: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// A fully-loaded past execution, including the records needed to undo it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredExecution {
    pub id: String,
    pub zone_id: String,
    pub created_at: String,
    pub applied: usize,
    pub skipped: usize,
    pub failed: usize,
    pub audit: AuditLog,
    pub undo: UndoJournal,
}

/// Persist an execution report, returning the generated id.
pub fn save_execution(
    conn: &Connection,
    zone_id: &str,
    report: &ExecutionReport,
) -> rusqlite::Result<String> {
    let id = Uuid::new_v4().to_string();
    let ts = now_ts();
    // The audit log / undo journal are pure data; serialization cannot realistically
    // fail, but surface any error as a SQLite-compatible error rather than panicking.
    let audit_json = serde_json::to_string(&report.audit).map_err(to_sqlite_err)?;
    let undo_json = serde_json::to_string(&report.undo).map_err(to_sqlite_err)?;
    conn.execute(
        "INSERT INTO organizer_executions \
         (id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            zone_id,
            ts,
            report.applied as i64,
            report.skipped as i64,
            report.failed as i64,
            audit_json,
            undo_json,
        ],
    )?;
    Ok(id)
}

/// List past executions, newest first.
pub fn list_executions(conn: &Connection) -> rusqlite::Result<Vec<ExecutionSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, zone_id, created_at, applied, skipped, failed \
         FROM organizer_executions ORDER BY created_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ExecutionSummary {
            id: row.get(0)?,
            zone_id: row.get(1)?,
            created_at: row.get(2)?,
            applied: row.get::<_, i64>(3)? as usize,
            skipped: row.get::<_, i64>(4)? as usize,
            failed: row.get::<_, i64>(5)? as usize,
        })
    })?;
    rows.collect()
}

/// Load a single past execution with its audit log and undo journal.
pub fn get_execution(conn: &Connection, id: &str) -> rusqlite::Result<Option<StoredExecution>> {
    let mut stmt = conn.prepare(
        "SELECT id, zone_id, created_at, applied, skipped, failed, audit_json, undo_json \
         FROM organizer_executions WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
        let audit_json: String = row.get(6)?;
        let undo_json: String = row.get(7)?;
        let audit = serde_json::from_str(&audit_json).map_err(to_sqlite_err)?;
        let undo = serde_json::from_str(&undo_json).map_err(to_sqlite_err)?;
        Ok(StoredExecution {
            id: row.get(0)?,
            zone_id: row.get(1)?,
            created_at: row.get(2)?,
            applied: row.get::<_, i64>(3)? as usize,
            skipped: row.get::<_, i64>(4)? as usize,
            failed: row.get::<_, i64>(5)? as usize,
            audit,
            undo,
        })
    })?;
    match rows.next() {
        Some(execution) => Ok(Some(execution?)),
        None => Ok(None),
    }
}

/// Map a JSON (de)serialization error into a rusqlite error so it threads
/// through `query_map` closures and the `?` operator cleanly.
fn to_sqlite_err(e: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organizer::executor::execute_plan;
    use crate::organizer::planner::plan_with_rules;
    use crate::policy::FolderRule;
    use crate::storage::open_in_memory;
    use std::path::{Path, PathBuf};

    fn full_rule(path: &Path) -> FolderRule {
        FolderRule {
            path: path.to_path_buf(),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
        }
    }

    /// A unique scratch directory for one test, cleaned up on drop. Local to this
    /// module because the organizer's own `testutil` is private to that module.
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("ghost-exec-test-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
        fn file(&self, name: &str, bytes: &[u8]) {
            std::fs::write(self.0.join(name), bytes).unwrap();
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn save_and_load_round_trips_audit_and_undo() {
        let tmp = Scratch::new();
        tmp.file("report.pdf", b"a");
        tmp.file("song.mp3", b"b");
        let rules = vec![full_rule(tmp.path())];

        let plan = plan_with_rules("z", &rules);
        let report = execute_plan(&plan, &rules);
        assert!(report.applied >= 2);

        let conn = open_in_memory().unwrap();
        let id = save_execution(&conn, "z", &report).unwrap();

        // Summary list reflects the run.
        let list = list_executions(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        assert_eq!(list[0].applied, report.applied);

        // Full load reconstructs the audit log and undo journal byte-for-byte.
        let loaded = get_execution(&conn, &id).unwrap().unwrap();
        assert_eq!(loaded.audit, report.audit);
        assert_eq!(loaded.undo, report.undo);
        assert_eq!(loaded.zone_id, "z");
    }

    #[test]
    fn get_execution_returns_none_for_unknown_id() {
        let conn = open_in_memory().unwrap();
        assert!(get_execution(&conn, "missing").unwrap().is_none());
    }

    #[test]
    fn list_executions_is_empty_on_a_fresh_database() {
        let conn = open_in_memory().unwrap();
        assert!(list_executions(&conn).unwrap().is_empty());
    }
}
