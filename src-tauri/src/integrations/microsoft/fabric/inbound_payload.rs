//! Parse inbound Fabric / Eventstream / CloudEvents webhook bodies into intents.

use serde::Deserialize;

/// Normalized fields for `triggers::record_intent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInboundIntent {
    pub zone_id: Option<String>,
    pub source: String,
    pub summary: String,
}

#[derive(Deserialize)]
struct GhostNativePayload {
    zone_id: Option<String>,
    source: Option<String>,
    summary: String,
}

#[derive(Deserialize)]
struct CloudEventsPayload {
    #[serde(rename = "type")]
    event_type: Option<String>,
    subject: Option<String>,
    source: Option<String>,
    data: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct FabricItemEventPayload {
    #[serde(rename = "type")]
    event_type: Option<String>,
    subject: Option<String>,
    source: Option<String>,
    data: Option<serde_json::Value>,
}

pub(crate) fn normalize_source(source: Option<&str>) -> String {
    match source.map(str::trim).filter(|s| !s.is_empty()) {
        Some("fabric-pipeline") => "ops.pipeline".to_string(),
        Some("eventstream-activator") => "hub.bridge".to_string(),
        Some(value) => value.to_string(),
        None => "hub.bridge".to_string(),
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

/// Accept Ghost-native JSON, CloudEvents 1.0, or Fabric item-event envelopes.
pub fn parse_inbound_body(body: &str) -> Result<ParsedInboundIntent, String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err("empty body".into());
    }

    if let Ok(native) = serde_json::from_str::<GhostNativePayload>(trimmed)
        && !native.summary.trim().is_empty()
    {
        return Ok(ParsedInboundIntent {
            zone_id: normalize_optional_string(native.zone_id),
            source: normalize_source(native.source.as_deref()),
            summary: native.summary.trim().to_string(),
        });
    }

    if let Ok(ce) = serde_json::from_str::<CloudEventsPayload>(trimmed)
        && (ce.event_type.is_some() || ce.data.is_some())
    {
        return Ok(from_cloud_events(ce));
    }

    if let Ok(fe) = serde_json::from_str::<FabricItemEventPayload>(trimmed)
        && (fe.event_type.is_some() || fe.subject.is_some())
    {
        return Ok(from_fabric_item_event(fe));
    }

    Err(
        "unrecognized webhook JSON — use Ghost native {\"summary\":\"...\"}, CloudEvents 1.0, or Fabric item event shape; see docs/fabric-eventstream-webhook.md"
            .into(),
    )
}

fn from_cloud_events(ce: CloudEventsPayload) -> ParsedInboundIntent {
    let event_type = ce.event_type.unwrap_or_else(|| "unknown.event".to_string());
    let source = normalize_source(ce.source.as_deref());
    let summary = if let Some(data) = ce.data {
        if let Some(s) = data.get("summary").and_then(|v| v.as_str()) {
            s.to_string()
        } else {
            format!("{event_type} — {}", data)
        }
    } else {
        format!(
            "{event_type}{}",
            ce.subject
                .as_ref()
                .map(|s| format!(" ({s})"))
                .unwrap_or_default()
        )
    };
    ParsedInboundIntent {
        zone_id: normalize_optional_string(ce.subject),
        source,
        summary,
    }
}

fn from_fabric_item_event(ev: FabricItemEventPayload) -> ParsedInboundIntent {
    let event_type = ev
        .event_type
        .unwrap_or_else(|| "Microsoft.Fabric.Event".to_string());
    let source = normalize_source(ev.source.as_deref());
    let summary = if let Some(data) = &ev.data {
        if let Some(s) = data.get("summary").and_then(|v| v.as_str()) {
            s.to_string()
        } else {
            format!("{event_type} — {data}")
        }
    } else {
        format!(
            "{event_type}{}",
            ev.subject
                .as_ref()
                .map(|s| format!(" — {s}"))
                .unwrap_or_default()
        )
    };
    ParsedInboundIntent {
        zone_id: normalize_optional_string(ev.subject),
        source,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_native_round_trip() {
        let parsed = parse_inbound_body(
            r#"{"zone_id":"z1","source":"ops.pipeline","summary":"Run finished"}"#,
        )
        .unwrap();
        assert_eq!(parsed.summary, "Run finished");
        assert_eq!(parsed.zone_id.as_deref(), Some("z1"));
    }

    #[test]
    fn ghost_native_pos_close_round_trip() {
        let parsed = parse_inbound_body(
            r#"{"zone_id":"shift-zone","source":"pos.close","summary":"Shift closed — file today's receipts"}"#,
        )
        .unwrap();
        assert_eq!(parsed.source, "pos.close");
        assert_eq!(parsed.zone_id.as_deref(), Some("shift-zone"));
    }

    #[test]
    fn legacy_sources_normalize_to_shared_taxonomy() {
        let parsed = parse_inbound_body(
            r#"{"source":"fabric-pipeline","summary":"Nightly export pipeline completed"}"#,
        )
        .unwrap();
        assert_eq!(parsed.source, "ops.pipeline");
    }

    #[test]
    fn cloudevents_maps_type_and_data() {
        let parsed = parse_inbound_body(
            r#"{"specversion":"1.0","type":"com.example.pipeline.done","source":"fabric","subject":"ws/item","data":{"summary":"Export ready"}}"#,
        )
        .unwrap();
        assert_eq!(parsed.summary, "Export ready");
        assert_eq!(parsed.source, "fabric");
    }

    #[test]
    fn fabric_item_event_without_data_uses_type() {
        let parsed = parse_inbound_body(
            r#"{"type":"Microsoft.Fabric.Capacity.Throttled","subject":"capacity/abc","source":"/subscriptions/..."}"#,
        )
        .unwrap();
        assert!(parsed.summary.contains("Throttled"));
    }

    #[test]
    fn docs_sample_payloads_parse() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let samples = [
            "docs/samples/fabric-eventstream/ghost-native.json",
            "docs/samples/fabric-eventstream/pos-shift-close.json",
            "docs/samples/fabric-eventstream/cloudevents-pipeline-done.json",
            "docs/samples/fabric-eventstream/fabric-item-event.json",
            "docs/samples/fabric-eventstream/eventstream-row-bridge.json",
        ];
        for rel in samples {
            let path = root.join(rel);
            let body = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("could not read {}: {}", path.display(), e));
            let parsed = parse_inbound_body(&body)
                .unwrap_or_else(|e| panic!("sample {rel} failed to parse: {e}"));
            assert!(
                !parsed.summary.trim().is_empty(),
                "sample {rel} must yield a non-empty summary"
            );
        }
    }
}
