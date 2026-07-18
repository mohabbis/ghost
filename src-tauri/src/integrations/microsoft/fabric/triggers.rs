//! Inbound Fabric intent queue — surfaces external signals without auto-executing.
//!
//! Inbound triggers must never bypass desktop approval. This module only records
//! intents for the user to review in Organizer, whether the bridge source is
//! Fabric, a batch pipeline, or a POS closeout event.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InboundIntentStatus {
    Pending,
    Dismissed,
    Converted,
    Expired,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FabricInboundIntent {
    pub intent_id: String,
    pub zone_id: Option<String>,
    pub source: String,
    pub summary: String,
    pub status: InboundIntentStatus,
    pub received_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Default, Serialize, Deserialize)]
struct Store {
    intents: HashMap<String, FabricInboundIntent>,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn normalize_source(source: &str) -> String {
    match source.trim() {
        "fabric-pipeline" => "ops.pipeline".to_string(),
        "eventstream-activator" => "hub.bridge".to_string(),
        value if !value.is_empty() => value.to_string(),
        _ => "hub.bridge".to_string(),
    }
}

fn store_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ghost")
        .join("fabric-inbound-intents.json")
}

fn with_store<F, T>(f: F) -> T
where
    F: FnOnce(&mut Store) -> T,
{
    let mut guard = STORE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let path = store_path();
        *guard = Some(if let Ok(bytes) = fs::read(&path) {
            serde_json::from_slice(&bytes).unwrap_or_default()
        } else {
            Store::default()
        });
    }
    let store = guard.get_or_insert_with(Store::default);
    refresh_expired(store);
    let result = f(store);
    if let Some(parent) = store_path().parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_vec(store) {
        let _ = fs::write(store_path(), json);
    }
    result
}

fn refresh_expired(store: &mut Store) {
    let now = Utc::now();
    for intent in store.intents.values_mut() {
        if intent.status == InboundIntentStatus::Pending && now >= intent.expires_at {
            intent.status = InboundIntentStatus::Expired;
        }
    }
}

pub fn record_intent(zone_id: Option<String>, source: &str, summary: &str) -> FabricInboundIntent {
    let now = Utc::now();
    let intent = FabricInboundIntent {
        intent_id: uuid::Uuid::new_v4().to_string(),
        zone_id,
        source: normalize_source(source),
        summary: summary.trim().to_string(),
        status: InboundIntentStatus::Pending,
        received_at: now,
        expires_at: now + Duration::hours(24),
    };
    with_store(|store| {
        store
            .intents
            .insert(intent.intent_id.clone(), intent.clone());
    });
    intent
}

pub fn list_pending() -> Vec<FabricInboundIntent> {
    with_store(|store| {
        let mut intents: Vec<_> = store
            .intents
            .values()
            .filter(|i| i.status == InboundIntentStatus::Pending)
            .cloned()
            .collect();
        intents.sort_by(|a, b| b.received_at.cmp(&a.received_at));
        intents
    })
}

pub fn get_intent(intent_id: &str) -> Result<FabricInboundIntent, String> {
    with_store(|store| {
        store
            .intents
            .get(intent_id)
            .cloned()
            .ok_or_else(|| format!("Unknown inbound intent '{intent_id}'"))
    })
}

pub fn dismiss_intent(intent_id: &str) -> Result<(), String> {
    with_store(|store| {
        let intent = store
            .intents
            .get_mut(intent_id)
            .ok_or_else(|| format!("Unknown inbound intent '{intent_id}'"))?;
        intent.status = InboundIntentStatus::Dismissed;
        Ok(())
    })
}

pub fn convert_intent(intent_id: &str) -> Result<FabricInboundIntent, String> {
    with_store(|store| {
        let intent = store
            .intents
            .get_mut(intent_id)
            .ok_or_else(|| format!("Unknown inbound intent '{intent_id}'"))?;
        if intent.status != InboundIntentStatus::Pending {
            return Err(format!(
                "Inbound intent '{intent_id}' is not pending (current status: {:?})",
                intent.status
            ));
        }
        intent.status = InboundIntentStatus::Converted;
        Ok(intent.clone())
    })
}

#[cfg(test)]
pub(crate) fn reset_store_for_tests() {
    let mut guard = STORE.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(Store::default());
    let _ = fs::remove_file(store_path());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_list_round_trip() {
        reset_store_for_tests();
        let intent = record_intent(Some("zone-1".into()), "ops.pipeline", "Pipeline completed");
        let pending = list_pending();
        assert!(pending.iter().any(|i| i.intent_id == intent.intent_id));
        reset_store_for_tests();
    }

    #[test]
    fn convert_marks_intent_and_hides_it_from_pending() {
        reset_store_for_tests();
        let intent = record_intent(Some("zone-1".into()), "pos.close", "Shift closed");
        let converted = convert_intent(&intent.intent_id).expect("convert intent");
        assert_eq!(converted.status, InboundIntentStatus::Converted);
        assert!(list_pending().is_empty());
        let stored = get_intent(&intent.intent_id).expect("stored intent");
        assert_eq!(stored.status, InboundIntentStatus::Converted);
        reset_store_for_tests();
    }
}
