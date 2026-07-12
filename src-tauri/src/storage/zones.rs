//! redb-backed repository for Zones and their folder rules.
//!
//! Persists the pure domain types from `crate::policy` (`Zone`, `FolderRule`)
//! so the policy engine can be fed real, user-approved boundaries.

use crate::policy::{DefaultDecision, FolderRule, TrustLevel, Zone};
use crate::storage::Db;
use redb::{ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

const ZONES: TableDefinition<&str, &[u8]> = TableDefinition::new("zones");
const FOLDER_RULES: TableDefinition<&str, &[u8]> = TableDefinition::new("zone_folder_rules");

/// Ensure both tables exist. Called once from `storage::open_default`/
/// `open_in_memory` right after the database is created.
pub(crate) fn init(write_txn: &WriteTransaction) -> anyhow::Result<()> {
    write_txn.open_table(ZONES)?;
    write_txn.open_table(FOLDER_RULES)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct ZoneRow {
    name: String,
    description: Option<String>,
    default_decision: String,
    rename_dated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct RuleRow {
    zone_id: String,
    path: String,
    can_read: bool,
    can_create: bool,
    can_rename: bool,
    can_move: bool,
    can_copy: bool,
    can_delete: bool,
    trust_level: String,
}

fn zone_from_row(id: &str, row: ZoneRow) -> Zone {
    Zone {
        id: id.to_string(),
        name: row.name,
        description: row.description,
        // An out-of-range token should never appear (only this module writes
        // it), but be conservative and fall back to the safest decision.
        default_decision: DefaultDecision::from_token(&row.default_decision)
            .unwrap_or(DefaultDecision::Deny),
        rename_dated: row.rename_dated,
    }
}

fn rule_from_row(row: RuleRow) -> FolderRule {
    FolderRule {
        path: PathBuf::from(row.path),
        can_read: row.can_read,
        can_create: row.can_create,
        can_rename: row.can_rename,
        can_move: row.can_move,
        can_copy: row.can_copy,
        can_delete: row.can_delete,
        trust: TrustLevel::from_token(&row.trust_level).unwrap_or(TrustLevel::AskFirst),
    }
}

/// Create a Zone, returning the stored record (with its generated id).
pub fn create_zone(
    db: &Db,
    name: &str,
    description: Option<&str>,
    default_decision: DefaultDecision,
    rename_dated: bool,
) -> anyhow::Result<Zone> {
    let id = Uuid::new_v4().to_string();
    let row = ZoneRow {
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
        default_decision: default_decision.as_str().to_string(),
        rename_dated,
    };
    let write_txn = db.db.begin_write()?;
    {
        let mut table = write_txn.open_table(ZONES)?;
        table.insert(id.as_str(), serde_json::to_vec(&row)?.as_slice())?;
    }
    write_txn.commit()?;
    Ok(zone_from_row(&id, row))
}

/// Create a Zone and attach a batch of folder rules atomically, in one write
/// transaction — used by policy-pack import so a partially-imported pack can
/// never leave a Zone with only some of its rules.
pub fn create_zone_with_rules(
    db: &Db,
    name: &str,
    description: Option<&str>,
    default_decision: DefaultDecision,
    rename_dated: bool,
    rules: &[FolderRule],
) -> anyhow::Result<Zone> {
    let id = Uuid::new_v4().to_string();
    let row = ZoneRow {
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
        default_decision: default_decision.as_str().to_string(),
        rename_dated,
    };
    let write_txn = db.db.begin_write()?;
    {
        let mut table = write_txn.open_table(ZONES)?;
        table.insert(id.as_str(), serde_json::to_vec(&row)?.as_slice())?;
    }
    for rule in rules {
        insert_folder_rule(&write_txn, &id, rule)?;
    }
    write_txn.commit()?;
    Ok(zone_from_row(&id, row))
}

/// Fetch a single Zone by id.
pub fn get_zone(db: &Db, id: &str) -> anyhow::Result<Option<Zone>> {
    let read_txn = db.db.begin_read()?;
    let table = read_txn.open_table(ZONES)?;
    match table.get(id)? {
        Some(guard) => Ok(Some(zone_from_row(
            id,
            serde_json::from_slice(guard.value())?,
        ))),
        None => Ok(None),
    }
}

/// List all Zones, ordered by name.
pub fn list_zones(db: &Db) -> anyhow::Result<Vec<Zone>> {
    let read_txn = db.db.begin_read()?;
    let table = read_txn.open_table(ZONES)?;
    let mut zones = Vec::new();
    for entry in table.iter()? {
        let (key, value) = entry?;
        let row: ZoneRow = serde_json::from_slice(value.value())?;
        zones.push(zone_from_row(key.value(), row));
    }
    zones.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(zones)
}

fn insert_folder_rule(
    write_txn: &WriteTransaction,
    zone_id: &str,
    rule: &FolderRule,
) -> anyhow::Result<()> {
    let id = Uuid::new_v4().to_string();
    let row = RuleRow {
        zone_id: zone_id.to_string(),
        path: rule.path.to_string_lossy().into_owned(),
        can_read: rule.can_read,
        can_create: rule.can_create,
        can_rename: rule.can_rename,
        can_move: rule.can_move,
        can_copy: rule.can_copy,
        can_delete: rule.can_delete,
        trust_level: rule.trust.as_str().to_string(),
    };
    let mut table = write_txn.open_table(FOLDER_RULES)?;
    table.insert(id.as_str(), serde_json::to_vec(&row)?.as_slice())?;
    Ok(())
}

/// Attach a folder rule to a Zone.
pub fn add_folder_rule(db: &Db, zone_id: &str, rule: &FolderRule) -> anyhow::Result<()> {
    let write_txn = db.db.begin_write()?;
    insert_folder_rule(&write_txn, zone_id, rule)?;
    write_txn.commit()?;
    Ok(())
}

/// Update the trust level of a Zone's folder rule identified by its path.
/// Returns the number of rows changed (0 if no such rule exists).
pub fn set_rule_trust(
    db: &Db,
    zone_id: &str,
    path: &str,
    trust: TrustLevel,
) -> anyhow::Result<usize> {
    let write_txn = db.db.begin_write()?;
    let mut changed = 0usize;
    {
        let mut table = write_txn.open_table(FOLDER_RULES)?;
        let matches: Vec<(String, RuleRow)> = {
            let mut found = Vec::new();
            for entry in table.iter()? {
                let (key, value) = entry?;
                let row: RuleRow = serde_json::from_slice(value.value())?;
                if row.zone_id == zone_id && row.path == path {
                    found.push((key.value().to_string(), row));
                }
            }
            found
        };
        for (id, mut row) in matches {
            row.trust_level = trust.as_str().to_string();
            table.insert(id.as_str(), serde_json::to_vec(&row)?.as_slice())?;
            changed += 1;
        }
    }
    write_txn.commit()?;
    Ok(changed)
}

/// List the folder rules for a Zone, ordered by path.
pub fn list_folder_rules(db: &Db, zone_id: &str) -> anyhow::Result<Vec<FolderRule>> {
    let read_txn = db.db.begin_read()?;
    let table = read_txn.open_table(FOLDER_RULES)?;
    let mut rules = Vec::new();
    for entry in table.iter()? {
        let (_key, value) = entry?;
        let row: RuleRow = serde_json::from_slice(value.value())?;
        if row.zone_id == zone_id {
            rules.push(rule_from_row(row));
        }
    }
    rules.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(rules)
}

/// Import a Zone at a known id (used only by the one-time SQLite migration —
/// see `storage::sqlite_import` — so imported records keep their original id).
pub(crate) fn import_zone_raw(
    write_txn: &WriteTransaction,
    id: &str,
    name: &str,
    description: Option<&str>,
    default_decision_token: &str,
    rename_dated: bool,
) -> anyhow::Result<()> {
    let row = ZoneRow {
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
        default_decision: default_decision_token.to_string(),
        rename_dated,
    };
    let mut table = write_txn.open_table(ZONES)?;
    table.insert(id, serde_json::to_vec(&row)?.as_slice())?;
    Ok(())
}

/// Import a folder rule at a known id (see [`import_zone_raw`]).
#[allow(clippy::too_many_arguments)]
pub(crate) fn import_folder_rule_raw(
    write_txn: &WriteTransaction,
    id: &str,
    zone_id: &str,
    path: &str,
    can_read: bool,
    can_create: bool,
    can_rename: bool,
    can_move: bool,
    can_copy: bool,
    can_delete: bool,
    trust_level: &str,
) -> anyhow::Result<()> {
    let row = RuleRow {
        zone_id: zone_id.to_string(),
        path: path.to_string(),
        can_read,
        can_create,
        can_rename,
        can_move,
        can_copy,
        can_delete,
        trust_level: trust_level.to_string(),
    };
    let mut table = write_txn.open_table(FOLDER_RULES)?;
    table.insert(id, serde_json::to_vec(&row)?.as_slice())?;
    Ok(())
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
        let db = open_in_memory().unwrap();
        let zone = create_zone(
            &db,
            "School",
            Some("coursework"),
            DefaultDecision::Ask,
            false,
        )
        .unwrap();
        add_folder_rule(&db, &zone.id, &FolderRule::read_only("/home/u/Downloads")).unwrap();

        let zones = list_zones(&db).unwrap();
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].name, "School");
        assert_eq!(zones[0].description.as_deref(), Some("coursework"));

        let got = get_zone(&db, &zone.id).unwrap().unwrap();
        assert_eq!(got.default_decision, DefaultDecision::Ask);

        let rules = list_folder_rules(&db, &zone.id).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].can_read);
        assert!(!rules[0].can_move);
    }

    #[test]
    fn loaded_rules_drive_policy_decisions() {
        let db = open_in_memory().unwrap();
        let zone = create_zone(&db, "School", None, DefaultDecision::Ask, false).unwrap();
        add_folder_rule(&db, &zone.id, &full_rule("/home/u/Downloads")).unwrap();
        add_folder_rule(&db, &zone.id, &full_rule("/home/u/Docs")).unwrap();

        let rules = list_folder_rules(&db, &zone.id).unwrap();

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
        let db = open_in_memory().unwrap();
        assert!(get_zone(&db, "does-not-exist").unwrap().is_none());
    }

    #[test]
    fn list_zones_is_empty_on_a_fresh_database() {
        let db = open_in_memory().unwrap();
        assert!(list_zones(&db).unwrap().is_empty());
    }

    #[test]
    fn folder_rules_are_empty_for_a_zone_without_rules() {
        let db = open_in_memory().unwrap();
        let zone = create_zone(&db, "Empty", None, DefaultDecision::Deny, false).unwrap();
        assert!(list_folder_rules(&db, &zone.id).unwrap().is_empty());
    }

    #[test]
    fn create_zone_without_description_round_trips_as_none() {
        let db = open_in_memory().unwrap();
        let zone = create_zone(&db, "NoDesc", None, DefaultDecision::Allow, false).unwrap();
        assert_eq!(zone.description, None);

        let loaded = get_zone(&db, &zone.id).unwrap().unwrap();
        assert_eq!(loaded.description, None);
        assert_eq!(loaded.default_decision, DefaultDecision::Allow);
    }

    #[test]
    fn list_zones_is_ordered_by_name() {
        let db = open_in_memory().unwrap();
        create_zone(&db, "Charlie", None, DefaultDecision::Ask, false).unwrap();
        create_zone(&db, "Alpha", None, DefaultDecision::Ask, false).unwrap();
        create_zone(&db, "Bravo", None, DefaultDecision::Ask, false).unwrap();

        let names: Vec<String> = list_zones(&db)
            .unwrap()
            .into_iter()
            .map(|z| z.name)
            .collect();
        assert_eq!(names, vec!["Alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn folder_rules_are_ordered_by_path() {
        let db = open_in_memory().unwrap();
        let zone = create_zone(&db, "Z", None, DefaultDecision::Ask, false).unwrap();
        add_folder_rule(&db, &zone.id, &FolderRule::read_only("/home/u/zeta")).unwrap();
        add_folder_rule(&db, &zone.id, &FolderRule::read_only("/home/u/alpha")).unwrap();

        let paths: Vec<PathBuf> = list_folder_rules(&db, &zone.id)
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
        let db = open_in_memory().unwrap();
        let a = create_zone(&db, "A", None, DefaultDecision::Ask, false).unwrap();
        let b = create_zone(&db, "B", None, DefaultDecision::Ask, false).unwrap();
        add_folder_rule(&db, &a.id, &FolderRule::read_only("/home/u/a")).unwrap();

        assert_eq!(list_folder_rules(&db, &a.id).unwrap().len(), 1);
        assert!(list_folder_rules(&db, &b.id).unwrap().is_empty());
    }

    #[test]
    fn set_rule_trust_updates_an_existing_rule() {
        let db = open_in_memory().unwrap();
        let zone = create_zone(&db, "Z", None, DefaultDecision::Ask, false).unwrap();
        add_folder_rule(&db, &zone.id, &full_rule("/home/u/a")).unwrap();

        let changed = set_rule_trust(&db, &zone.id, "/home/u/a", TrustLevel::Automate).unwrap();
        assert_eq!(changed, 1);
        let rules = list_folder_rules(&db, &zone.id).unwrap();
        assert_eq!(rules[0].trust, TrustLevel::Automate);

        // A path that doesn't match changes nothing.
        let none = set_rule_trust(&db, &zone.id, "/home/u/nope", TrustLevel::Never).unwrap();
        assert_eq!(none, 0);
    }

    /// Trust levels persist and load back exactly; a rule stored without an
    /// explicit level (pre-V4 rows get the column default) reads as ask_first.
    #[test]
    fn trust_level_round_trips() {
        let db = open_in_memory().unwrap();
        let zone = create_zone(&db, "Z", None, DefaultDecision::Ask, false).unwrap();
        let automated = FolderRule {
            trust: TrustLevel::Automate,
            ..full_rule("/home/u/a")
        };
        let never = FolderRule {
            trust: TrustLevel::Never,
            ..full_rule("/home/u/b")
        };
        add_folder_rule(&db, &zone.id, &automated).unwrap();
        add_folder_rule(&db, &zone.id, &never).unwrap();
        add_folder_rule(&db, &zone.id, &FolderRule::read_only("/home/u/c")).unwrap();

        let rules = list_folder_rules(&db, &zone.id).unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].trust, TrustLevel::Automate);
        assert_eq!(rules[1].trust, TrustLevel::Never);
        assert_eq!(rules[2].trust, TrustLevel::AskFirst);
    }

    #[test]
    fn rename_dated_round_trips() {
        let db = open_in_memory().unwrap();
        create_zone(&db, "Client filing", None, DefaultDecision::Ask, true).unwrap();

        let zones = list_zones(&db).unwrap();
        assert_eq!(zones.len(), 1);
        assert!(zones[0].rename_dated);

        let loaded = get_zone(&db, &zones[0].id).unwrap().unwrap();
        assert!(loaded.rename_dated);
    }

    #[test]
    fn create_zone_with_rules_is_atomic_and_round_trips() {
        let db = open_in_memory().unwrap();
        let rules = vec![full_rule("/home/u/a"), full_rule("/home/u/b")];
        let zone =
            create_zone_with_rules(&db, "Pack", Some("d"), DefaultDecision::Ask, true, &rules)
                .unwrap();

        assert_eq!(get_zone(&db, &zone.id).unwrap().unwrap().name, "Pack");
        assert_eq!(list_folder_rules(&db, &zone.id).unwrap().len(), 2);
    }
}
