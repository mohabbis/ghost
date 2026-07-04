//! SQLite-backed repository for Zones and their folder rules.
//!
//! Persists the pure domain types from `crate::policy` (`Zone`, `FolderRule`)
//! so the policy engine can be fed real, user-approved boundaries.

use crate::policy::{DefaultDecision, FolderRule, TrustLevel, Zone};
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
    rename_dated: bool,
) -> rusqlite::Result<Zone> {
    let id = Uuid::new_v4().to_string();
    let ts = now_ts();
    conn.execute(
        "INSERT INTO zones \
         (id, name, description, default_decision, rename_dated, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![
            id,
            name,
            description,
            default_decision.as_str(),
            rename_dated as i64,
            ts
        ],
    )?;
    Ok(Zone {
        id,
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
        default_decision,
        rename_dated,
    })
}

/// Fetch a single Zone by id.
pub fn get_zone(conn: &Connection, id: &str) -> rusqlite::Result<Option<Zone>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, default_decision, rename_dated FROM zones WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], row_to_zone)?;
    match rows.next() {
        Some(zone) => Ok(Some(zone?)),
        None => Ok(None),
    }
}

/// List all Zones, ordered by name.
pub fn list_zones(conn: &Connection) -> rusqlite::Result<Vec<Zone>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, default_decision, rename_dated FROM zones ORDER BY name",
    )?;
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
         (id, zone_id, path, can_read, can_create, can_rename, can_move, can_copy, can_delete, \
          trust_level) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
            rule.trust.as_str(),
        ],
    )?;
    Ok(())
}

/// Update the trust level of a Zone's folder rule identified by its path.
/// Returns the number of rows changed (0 if no such rule exists).
pub fn set_rule_trust(
    conn: &Connection,
    zone_id: &str,
    path: &str,
    trust: TrustLevel,
) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE zone_folder_rules SET trust_level = ?1 WHERE zone_id = ?2 AND path = ?3",
        params![trust.as_str(), zone_id, path],
    )
}

/// List the folder rules for a Zone, ordered by path.
pub fn list_folder_rules(conn: &Connection, zone_id: &str) -> rusqlite::Result<Vec<FolderRule>> {
    let mut stmt = conn.prepare(
        "SELECT path, can_read, can_create, can_rename, can_move, can_copy, can_delete, \
         trust_level \
         FROM zone_folder_rules WHERE zone_id = ?1 ORDER BY path",
    )?;
    let rows = stmt.query_map(params![zone_id], |row| {
        let trust_token: String = row.get(7)?;
        Ok(FolderRule {
            path: PathBuf::from(row.get::<_, String>(0)?),
            can_read: row.get::<_, i64>(1)? != 0,
            can_create: row.get::<_, i64>(2)? != 0,
            can_rename: row.get::<_, i64>(3)? != 0,
            can_move: row.get::<_, i64>(4)? != 0,
            can_copy: row.get::<_, i64>(5)? != 0,
            can_delete: row.get::<_, i64>(6)? != 0,
            // The CHECK constraint should make this infallible; fall back to
            // the most cautious mutating stance if a stray token appears.
            trust: TrustLevel::from_token(&trust_token).unwrap_or(TrustLevel::AskFirst),
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
        rename_dated: row.get::<_, i64>(4)? != 0,
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
            trust: crate::policy::TrustLevel::AskFirst,
        }
    }

    #[test]
    fn zone_and_rules_round_trip() {
        let conn = open_in_memory().unwrap();
        let zone = create_zone(
            &conn,
            "School",
            Some("coursework"),
            DefaultDecision::Ask,
            false,
        )
        .unwrap();
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
        let zone = create_zone(&conn, "School", None, DefaultDecision::Ask, false).unwrap();
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
        let zone = create_zone(&conn, "Empty", None, DefaultDecision::Deny, false).unwrap();
        assert!(list_folder_rules(&conn, &zone.id).unwrap().is_empty());
    }

    #[test]
    fn create_zone_without_description_round_trips_as_none() {
        let conn = open_in_memory().unwrap();
        let zone = create_zone(&conn, "NoDesc", None, DefaultDecision::Allow, false).unwrap();
        assert_eq!(zone.description, None);

        let loaded = get_zone(&conn, &zone.id).unwrap().unwrap();
        assert_eq!(loaded.description, None);
        assert_eq!(loaded.default_decision, DefaultDecision::Allow);
    }

    #[test]
    fn list_zones_is_ordered_by_name() {
        let conn = open_in_memory().unwrap();
        create_zone(&conn, "Charlie", None, DefaultDecision::Ask, false).unwrap();
        create_zone(&conn, "Alpha", None, DefaultDecision::Ask, false).unwrap();
        create_zone(&conn, "Bravo", None, DefaultDecision::Ask, false).unwrap();

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
        let zone = create_zone(&conn, "Z", None, DefaultDecision::Ask, false).unwrap();
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
        let a = create_zone(&conn, "A", None, DefaultDecision::Ask, false).unwrap();
        let b = create_zone(&conn, "B", None, DefaultDecision::Ask, false).unwrap();
        add_folder_rule(&conn, &a.id, &FolderRule::read_only("/home/u/a")).unwrap();

        assert_eq!(list_folder_rules(&conn, &a.id).unwrap().len(), 1);
        assert!(list_folder_rules(&conn, &b.id).unwrap().is_empty());
    }

    #[test]
    fn set_rule_trust_updates_an_existing_rule() {
        let conn = open_in_memory().unwrap();
        let zone = create_zone(&conn, "Z", None, DefaultDecision::Ask, false).unwrap();
        add_folder_rule(&conn, &zone.id, &full_rule("/home/u/a")).unwrap();

        let changed = set_rule_trust(&conn, &zone.id, "/home/u/a", TrustLevel::Automate).unwrap();
        assert_eq!(changed, 1);
        let rules = list_folder_rules(&conn, &zone.id).unwrap();
        assert_eq!(rules[0].trust, TrustLevel::Automate);

        // A path that doesn't match changes nothing.
        let none = set_rule_trust(&conn, &zone.id, "/home/u/nope", TrustLevel::Never).unwrap();
        assert_eq!(none, 0);
    }

    /// Trust levels persist and load back exactly; a rule stored without an
    /// explicit level (pre-V4 rows get the column default) reads as ask_first.
    #[test]
    fn trust_level_round_trips() {
        let conn = open_in_memory().unwrap();
        let zone = create_zone(&conn, "Z", None, DefaultDecision::Ask, false).unwrap();
        let automated = FolderRule {
            trust: TrustLevel::Automate,
            ..full_rule("/home/u/a")
        };
        let never = FolderRule {
            trust: TrustLevel::Never,
            ..full_rule("/home/u/b")
        };
        add_folder_rule(&conn, &zone.id, &automated).unwrap();
        add_folder_rule(&conn, &zone.id, &never).unwrap();
        add_folder_rule(&conn, &zone.id, &FolderRule::read_only("/home/u/c")).unwrap();

        let rules = list_folder_rules(&conn, &zone.id).unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].trust, TrustLevel::Automate);
        assert_eq!(rules[1].trust, TrustLevel::Never);
        assert_eq!(rules[2].trust, TrustLevel::AskFirst);
    }

    #[test]
    fn rename_dated_round_trips() {
        let conn = open_in_memory().unwrap();
        create_zone(&conn, "Client filing", None, DefaultDecision::Ask, true).unwrap();

        let zones = list_zones(&conn).unwrap();
        assert_eq!(zones.len(), 1);
        assert!(zones[0].rename_dated);

        let loaded = get_zone(&conn, &zones[0].id).unwrap().unwrap();
        assert!(loaded.rename_dated);
    }
}
