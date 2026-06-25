//! SQLite-backed repository for Zones and their folder rules.
//!
//! Persists the pure domain types from `crate::policy` (`Zone`, `FolderRule`)
//! so the policy engine can be fed real, user-approved boundaries.

use crate::policy::{DefaultDecision, FolderRule, Zone};
use rusqlite::{params, Connection, Row};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Epoch-seconds timestamp as a string. Avoids pulling in a date crate; good
/// enough for created/updated bookkeeping. Never panics on clock skew.
fn now_ts() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

/// Create a Zone, returning the stored record (with its generated id).
pub fn create_zone(
    conn: &Connection,
    name: &str,
    description: Option<&str>,
    default_decision: DefaultDecision,
) -> rusqlite::Result<Zone> {
    let id = Uuid::new_v4().to_string();
    let ts = now_ts();
    conn.execute(
        "INSERT INTO zones (id, name, description, default_decision, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, name, description, default_decision.as_str(), ts],
    )?;
    Ok(Zone {
        id,
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
        default_decision,
    })
}

/// Fetch a single Zone by id.
pub fn get_zone(conn: &Connection, id: &str) -> rusqlite::Result<Option<Zone>> {
    let mut stmt =
        conn.prepare("SELECT id, name, description, default_decision FROM zones WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![id], row_to_zone)?;
    match rows.next() {
        Some(zone) => Ok(Some(zone?)),
        None => Ok(None),
    }
}

/// List all Zones, ordered by name.
pub fn list_zones(conn: &Connection) -> rusqlite::Result<Vec<Zone>> {
    let mut stmt =
        conn.prepare("SELECT id, name, description, default_decision FROM zones ORDER BY name")?;
    let rows = stmt.query_map([], row_to_zone)?;
    rows.collect()
}

/// Attach a folder rule to a Zone.
pub fn add_folder_rule(
    conn: &Connection,
    zone_id: &str,
    rule: &FolderRule,
) -> rusqlite::Result<()> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO zone_folder_rules \
         (id, zone_id, path, can_read, can_create, can_rename, can_move, can_copy, can_delete) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            zone_id,
            rule.path.to_string_lossy(),
            rule.can_read as i64,
            rule.can_create as i64,
            rule.can_rename as i64,
            rule.can_move as i64,
            rule.can_copy as i64,
            rule.can_delete as i64,
        ],
    )?;
    Ok(())
}

/// List the folder rules for a Zone, ordered by path.
pub fn list_folder_rules(conn: &Connection, zone_id: &str) -> rusqlite::Result<Vec<FolderRule>> {
    let mut stmt = conn.prepare(
        "SELECT path, can_read, can_create, can_rename, can_move, can_copy, can_delete \
         FROM zone_folder_rules WHERE zone_id = ?1 ORDER BY path",
    )?;
    let rows = stmt.query_map(params![zone_id], |row| {
        Ok(FolderRule {
            path: PathBuf::from(row.get::<_, String>(0)?),
            can_read: row.get::<_, i64>(1)? != 0,
            can_create: row.get::<_, i64>(2)? != 0,
            can_rename: row.get::<_, i64>(3)? != 0,
            can_move: row.get::<_, i64>(4)? != 0,
            can_copy: row.get::<_, i64>(5)? != 0,
            can_delete: row.get::<_, i64>(6)? != 0,
        })
    })?;
    rows.collect()
}

fn row_to_zone(row: &Row) -> rusqlite::Result<Zone> {
    let token: String = row.get(3)?;
    Ok(Zone {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        // An out-of-range token should never appear (CHECK constraint), but be
        // conservative and fall back to the safest decision.
        default_decision: DefaultDecision::from_token(&token).unwrap_or(DefaultDecision::Deny),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{self, Capability, PolicyDecision, RiskLevel};
    use crate::storage::open_in_memory;

    fn full_rule(path: &str) -> FolderRule {
        FolderRule {
            path: PathBuf::from(path),
            can_read: true,
            can_create: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_delete: false,
        }
    }

    #[test]
    fn zone_and_rules_round_trip() {
        let conn = open_in_memory().unwrap();
        let zone = create_zone(&conn, "School", Some("coursework"), DefaultDecision::Ask).unwrap();
        add_folder_rule(&conn, &zone.id, &FolderRule::read_only("/home/u/Downloads")).unwrap();

        let zones = list_zones(&conn).unwrap();
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].name, "School");
        assert_eq!(zones[0].description.as_deref(), Some("coursework"));

        let got = get_zone(&conn, &zone.id).unwrap().unwrap();
        assert_eq!(got.default_decision, DefaultDecision::Ask);

        let rules = list_folder_rules(&conn, &zone.id).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].can_read);
        assert!(!rules[0].can_move);
    }

    #[test]
    fn loaded_rules_drive_policy_decisions() {
        let conn = open_in_memory().unwrap();
        let zone = create_zone(&conn, "School", None, DefaultDecision::Ask).unwrap();
        add_folder_rule(&conn, &zone.id, &full_rule("/home/u/Downloads")).unwrap();
        add_folder_rule(&conn, &zone.id, &full_rule("/home/u/Docs")).unwrap();

        let rules = list_folder_rules(&conn, &zone.id).unwrap();

        // Read inside an approved folder -> Allow.
        assert_eq!(
            policy::evaluate(
                &Capability::ReadFolder {
                    path: PathBuf::from("/home/u/Downloads/x")
                },
                &rules,
            ),
            PolicyDecision::Allow
        );

        // In-boundary move -> RequireConfirmation(Medium).
        match policy::evaluate(
            &Capability::MoveFile {
                from: PathBuf::from("/home/u/Downloads/a.pdf"),
                to: PathBuf::from("/home/u/Docs/a.pdf"),
            },
            &rules,
        ) {
            PolicyDecision::RequireConfirmation { risk, .. } => assert_eq!(risk, RiskLevel::Medium),
            other => panic!("expected confirmation, got {other:?}"),
        }

        // Delete -> Deny (MVP never deletes).
        assert!(policy::evaluate(
            &Capability::DeleteFile {
                path: PathBuf::from("/home/u/Downloads/a.pdf")
            },
            &rules,
        )
        .is_denied());

        // Out-of-boundary move -> Deny.
        assert!(policy::evaluate(
            &Capability::MoveFile {
                from: PathBuf::from("/home/u/Downloads/a.pdf"),
                to: PathBuf::from("/tmp/a.pdf"),
            },
            &rules,
        )
        .is_denied());
    }

    #[test]
    fn get_zone_returns_none_for_unknown_id() {
        let conn = open_in_memory().unwrap();
        assert!(get_zone(&conn, "does-not-exist").unwrap().is_none());
    }

    #[test]
    fn list_zones_is_empty_on_a_fresh_database() {
        let conn = open_in_memory().unwrap();
        assert!(list_zones(&conn).unwrap().is_empty());
    }

    #[test]
    fn folder_rules_are_empty_for_a_zone_without_rules() {
        let conn = open_in_memory().unwrap();
        let zone = create_zone(&conn, "Empty", None, DefaultDecision::Deny).unwrap();
        assert!(list_folder_rules(&conn, &zone.id).unwrap().is_empty());
    }

    #[test]
    fn create_zone_without_description_round_trips_as_none() {
        let conn = open_in_memory().unwrap();
        let zone = create_zone(&conn, "NoDesc", None, DefaultDecision::Allow).unwrap();
        assert_eq!(zone.description, None);

        let loaded = get_zone(&conn, &zone.id).unwrap().unwrap();
        assert_eq!(loaded.description, None);
        assert_eq!(loaded.default_decision, DefaultDecision::Allow);
    }

    #[test]
    fn list_zones_is_ordered_by_name() {
        let conn = open_in_memory().unwrap();
        create_zone(&conn, "Charlie", None, DefaultDecision::Ask).unwrap();
        create_zone(&conn, "Alpha", None, DefaultDecision::Ask).unwrap();
        create_zone(&conn, "Bravo", None, DefaultDecision::Ask).unwrap();

        let names: Vec<String> = list_zones(&conn)
            .unwrap()
            .into_iter()
            .map(|z| z.name)
            .collect();
        assert_eq!(names, vec!["Alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn folder_rules_are_ordered_by_path() {
        let conn = open_in_memory().unwrap();
        let zone = create_zone(&conn, "Z", None, DefaultDecision::Ask).unwrap();
        add_folder_rule(&conn, &zone.id, &FolderRule::read_only("/home/u/zeta")).unwrap();
        add_folder_rule(&conn, &zone.id, &FolderRule::read_only("/home/u/alpha")).unwrap();

        let paths: Vec<PathBuf> = list_folder_rules(&conn, &zone.id)
            .unwrap()
            .into_iter()
            .map(|r| r.path)
            .collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/home/u/alpha"),
                PathBuf::from("/home/u/zeta")
            ]
        );
    }

    #[test]
    fn folder_rules_are_scoped_to_their_zone() {
        let conn = open_in_memory().unwrap();
        let a = create_zone(&conn, "A", None, DefaultDecision::Ask).unwrap();
        let b = create_zone(&conn, "B", None, DefaultDecision::Ask).unwrap();
        add_folder_rule(&conn, &a.id, &FolderRule::read_only("/home/u/a")).unwrap();

        assert_eq!(list_folder_rules(&conn, &a.id).unwrap().len(), 1);
        assert!(list_folder_rules(&conn, &b.id).unwrap().is_empty());
    }
}
