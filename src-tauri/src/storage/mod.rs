//! Local SQLite persistence for trust boundaries (Zones + folder rules).
//!
//! The database lives next to the workflow JSON files, at
//! `<data_dir>/ghost/ghost.db`. Schema changes go through versioned
//! [`migrations`]. Domain types come from `crate::policy`.

use rusqlite::Connection;
use std::path::PathBuf;

pub mod executions;
pub mod migrations;
pub mod milestones;
pub mod zones;

/// Resolve the on-disk database path, creating the parent directory.
pub fn default_db_path() -> anyhow::Result<PathBuf> {
    let dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?
        .join("ghost");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("ghost.db"))
}

/// Open (creating if needed) the Ghost database and run migrations.
pub fn open_default() -> anyhow::Result<Connection> {
    let conn = Connection::open(default_db_path()?)?;
    migrations::migrate(&conn)?;
    Ok(conn)
}

/// Open a fresh in-memory database with migrations applied (test helper).
#[cfg(test)]
pub fn open_in_memory() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    migrations::migrate(&conn)?;
    Ok(conn)
}
