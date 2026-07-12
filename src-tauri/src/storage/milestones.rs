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

use crate::storage::Db;
use redb::{ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction};
use std::time::{SystemTime, UNIX_EPOCH};

const MILESTONES: TableDefinition<&str, &str> = TableDefinition::new("organizer_milestones");

/// Ensure the table exists. Called once from `storage::open_default`/
/// `open_in_memory` right after the database is created.
pub(crate) fn init(write_txn: &WriteTransaction) -> anyhow::Result<()> {
    write_txn.open_table(MILESTONES)?;
    Ok(())
}

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
pub fn record_milestone(db: &Db, milestone: Milestone) -> anyhow::Result<()> {
    let write_txn = db.db.begin_write()?;
    {
        let mut table = write_txn.open_table(MILESTONES)?;
        if table.get(milestone.key())?.is_none() {
            table.insert(milestone.key(), now_ts().as_str())?;
        }
    }
    write_txn.commit()?;
    Ok(())
}

/// The recorded epoch-seconds string for a milestone, if it has been reached.
pub fn get_milestone(db: &Db, milestone: Milestone) -> anyhow::Result<Option<String>> {
    let read_txn = db.db.begin_read()?;
    let table = read_txn.open_table(MILESTONES)?;
    Ok(table.get(milestone.key())?.map(|g| g.value().to_string()))
}

/// All recorded milestones as `(key, epoch_seconds)` pairs, for the local
/// diagnostics/telemetry export. Never includes anything but timestamps.
/// redb's `&str` keys iterate in byte-lexicographic order, matching the
/// original `ORDER BY key`.
pub fn list_milestones(db: &Db) -> anyhow::Result<Vec<(String, String)>> {
    let read_txn = db.db.begin_read()?;
    let table = read_txn.open_table(MILESTONES)?;
    let mut out = Vec::new();
    for entry in table.iter()? {
        let (k, v) = entry?;
        out.push((k.value().to_string(), v.value().to_string()));
    }
    Ok(out)
}

/// Import a milestone at its original key/timestamp (used only by the
/// one-time SQLite migration — see `storage::sqlite_import`).
pub(crate) fn import_milestone_raw(
    write_txn: &WriteTransaction,
    key: &str,
    at: &str,
) -> anyhow::Result<()> {
    let mut table = write_txn.open_table(MILESTONES)?;
    table.insert(key, at)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory;

    #[test]
    fn milestone_is_recorded_and_read_back() {
        let db = open_in_memory().unwrap();
        assert!(get_milestone(&db, Milestone::FirstPlan).unwrap().is_none());
        record_milestone(&db, Milestone::FirstPlan).unwrap();
        assert!(get_milestone(&db, Milestone::FirstPlan).unwrap().is_some());
    }

    #[test]
    fn recording_twice_keeps_the_first_timestamp() {
        let db = open_in_memory().unwrap();
        record_milestone(&db, Milestone::FirstRun).unwrap();
        let first = get_milestone(&db, Milestone::FirstRun).unwrap().unwrap();
        // A second record must not overwrite the original.
        {
            let write_txn = db.db.begin_write().unwrap();
            {
                let mut table = write_txn.open_table(MILESTONES).unwrap();
                table.insert("first_run", "1").unwrap();
            }
            write_txn.commit().unwrap();
        }
        record_milestone(&db, Milestone::FirstRun).unwrap();
        let after = get_milestone(&db, Milestone::FirstRun).unwrap().unwrap();
        // The manual value stuck: proof `record_milestone` didn't rewrite it.
        assert_eq!(after, "1");
        assert_ne!(first, "0");
    }

    #[test]
    fn list_milestones_returns_all_recorded_keys() {
        let db = open_in_memory().unwrap();
        record_milestone(&db, Milestone::FirstZoneCreated).unwrap();
        record_milestone(&db, Milestone::FirstUndo).unwrap();
        let all = list_milestones(&db).unwrap();
        let keys: Vec<&str> = all.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"first_zone_created"));
        assert!(keys.contains(&"first_undo"));
        assert_eq!(all.len(), 2);
    }
}
