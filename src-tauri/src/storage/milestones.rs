//! Local, first-touch milestone timestamps for time-to-value measurement.
//!
//! Ghost's positioning promises a fast, safe first cleanup. To let the product
//! honestly report how long that actually takes, the app records the epoch
//! seconds at which each milestone in the Organizer trust pipeline was first
//! reached — creating a Zone, previewing a plan, approving a run, running an
//! undo. This is **local-only bookkeeping**: nothing leaves the machine, and it
//! surfaces only through the existing local diagnostics/telemetry export.
//!
//! Each milestone is recorded at most once (first occurrence wins), so the
//! stored value is the moment the user first reached that step — the basis for
//! a "time to first safe cleanup" measurement taken entirely on-device.

use rusqlite::{params, Connection, OptionalExtension};
use std::time::{SystemTime, UNIX_EPOCH};

/// A named step in the Organizer trust pipeline whose first occurrence is
/// worth timing. Stored as its stable string key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Milestone {
    /// The user approved their first Zone (a trust boundary).
    FirstZoneCreated,
    /// The user previewed their first plan.
    FirstPlan,
    /// The user approved and ran their first execution.
    FirstRun,
    /// The user reversed a run for the first time.
    FirstUndo,
}

impl Milestone {
    /// The stable key stored in the `organizer_milestones` table.
    pub fn key(&self) -> &'static str {
        match self {
            Milestone::FirstZoneCreated => "first_zone_created",
            Milestone::FirstPlan => "first_plan",
            Milestone::FirstRun => "first_run",
            Milestone::FirstUndo => "first_undo",
        }
    }
}

/// Epoch-seconds timestamp as a string, matching the rest of the storage layer.
fn now_ts() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

/// Record that a milestone was reached, keeping only the first occurrence.
/// Idempotent: later calls for the same milestone leave the original time.
pub fn record_milestone(conn: &Connection, milestone: Milestone) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO organizer_milestones (key, at) VALUES (?1, ?2)",
        params![milestone.key(), now_ts()],
    )?;
    Ok(())
}

/// The recorded epoch-seconds string for a milestone, if it has been reached.
pub fn get_milestone(conn: &Connection, milestone: Milestone) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT at FROM organizer_milestones WHERE key = ?1",
        params![milestone.key()],
        |row| row.get(0),
    )
    .optional()
}

/// All recorded milestones as `(key, epoch_seconds)` pairs, for the local
/// diagnostics/telemetry export. Never includes anything but timestamps.
pub fn list_milestones(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT key, at FROM organizer_milestones ORDER BY key")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory;

    #[test]
    fn milestone_is_recorded_and_read_back() {
        let conn = open_in_memory().unwrap();
        assert!(get_milestone(&conn, Milestone::FirstPlan)
            .unwrap()
            .is_none());
        record_milestone(&conn, Milestone::FirstPlan).unwrap();
        assert!(get_milestone(&conn, Milestone::FirstPlan)
            .unwrap()
            .is_some());
    }

    #[test]
    fn recording_twice_keeps_the_first_timestamp() {
        let conn = open_in_memory().unwrap();
        record_milestone(&conn, Milestone::FirstRun).unwrap();
        let first = get_milestone(&conn, Milestone::FirstRun).unwrap().unwrap();
        // A second record must not overwrite the original.
        conn.execute(
            "UPDATE organizer_milestones SET at = '1' WHERE key = 'first_run'",
            [],
        )
        .unwrap();
        record_milestone(&conn, Milestone::FirstRun).unwrap();
        let after = get_milestone(&conn, Milestone::FirstRun).unwrap().unwrap();
        // INSERT OR IGNORE left our manual value in place: proof it didn't rewrite.
        assert_eq!(after, "1");
        assert_ne!(first, "0");
    }

    #[test]
    fn list_milestones_returns_all_recorded_keys() {
        let conn = open_in_memory().unwrap();
        record_milestone(&conn, Milestone::FirstZoneCreated).unwrap();
        record_milestone(&conn, Milestone::FirstUndo).unwrap();
        let all = list_milestones(&conn).unwrap();
        let keys: Vec<&str> = all.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"first_zone_created"));
        assert!(keys.contains(&"first_undo"));
        assert_eq!(all.len(), 2);
    }
}
