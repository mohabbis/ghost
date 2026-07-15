# Fabric Eventstream → Ghost Webhook Bridge

Ghost accepts inbound **intents only** at `POST /fabric/webhook` on the MCP HTTP server. Nothing auto-executes — intents surface in Organizer for scan, review, and approval.

## Important: Eventstream has no native HTTP push

Microsoft Fabric Eventstream custom endpoints speak **Kafka, AMQP 1.0, or Azure Event Hubs** — not arbitrary HTTP webhooks to your desktop. See [Add a custom endpoint source](https://learn.microsoft.com/en-us/fabric/real-time-intelligence/event-streams/add-source-custom-app).

To reach Ghost you need a **bridge** that converts stream or Fabric events into HTTP POSTs.

```text
Fabric Eventstream / Activator / Real-Time Hub
        |
        v
Bridge (Logic App, Power Automate, Azure Function, Reflex custom action)
        |
        |  HTTPS POST /fabric/webhook
        v
Ghost MCP HTTP server
        |
        v
fabric inbound intent queue → Organizer banner
```

## Ghost webhook contract

```http
POST /fabric/webhook HTTP/1.1
Host: your-ghost-host:8787
X-Ghost-Webhook-Secret: <from fabric_set_webhook_secret>
Content-Type: application/json
```

Ghost accepts three JSON shapes (see samples in `docs/samples/fabric-eventstream/`):

1. **Ghost native** — simplest for custom bridges
2. **CloudEvents 1.0** — Event Grid, many Azure connectors
3. **Fabric item event** — `type` + `subject` + optional `data` (Real-Time Hub / workspace events forwarded by a bridge)

## Sample payloads

### Ghost native (recommended for custom bridges)

`docs/samples/fabric-eventstream/ghost-native.json`

```json
{
  "zone_id": "organizer-zone-uuid",
  "source": "fabric-pipeline",
  "summary": "Nightly export pipeline completed — review Organizer plan"
}
```

### CloudEvents 1.0 (Event Grid / Logic Apps)

`docs/samples/fabric-eventstream/cloudevents-pipeline-done.json`

```json
{
  "specversion": "1.0",
  "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "type": "com.contoso.fabric.pipeline.completed",
  "source": "/subscriptions/.../workspaces/ws-001",
  "subject": "lakehouses/lh-001",
  "time": "2026-07-12T14:32:00Z",
  "datacontenttype": "application/json",
  "data": {
    "summary": "Pipeline run succeeded — 1,240 rows landed",
    "workspaceId": "ws-001",
    "itemId": "lh-001"
  }
}
```

Ghost uses `data.summary` when present; otherwise builds a summary from `type` and `subject`.

### Fabric capacity / workspace item event

`docs/samples/fabric-eventstream/fabric-item-event.json`

```json
{
  "type": "Microsoft.Fabric.Capacity.Throttled",
  "subject": "capacities/abc123",
  "source": "/subscriptions/00000000-0000-0000-0000-000000000000",
  "data": {
    "summary": "Capacity throttled — defer Organizer run"
  }
}
```

### Eventstream-derived row (after manageFields)

When an Eventstream operator selects fields, a bridge (Azure Function) can map a row like this:

`docs/samples/fabric-eventstream/eventstream-row-bridge.json`

```json
{
  "zone_id": null,
  "source": "eventstream-activator",
  "summary": "Avg temperature 82.4 exceeded threshold in 5m window (device-7)"
}
```

## Bridge patterns

### Power Automate / Activator custom action

1. Create an Activator rule on Eventstream or Real-Time Hub events.
2. Action type: **Power Automate flow** with an **HTTP** action.
3. POST to `https://<ghost-host>/fabric/webhook` with the secret header.
4. Body: Ghost native JSON or map Activator `triggerBody()` fields into `summary`.

See [Activator trigger Power Automate flows](https://learn.microsoft.com/en-us/fabric/real-time-intelligence/data-activator/activator-trigger-power-automate-flows).

### Azure Function (Event Grid → Eventstream pattern)

Microsoft documents CloudEvents → Eventstream via a decoder function when Eventstream cannot ingest CloudEvents directly ([example](https://github.com/sandervandevelde/WagoEventDecoderFunctionApp)). Use the same function (or a second HTTP action) to **also** POST a Ghost-native payload to `/fabric/webhook`.

### Eventstream HTTP source (preview)

Fabric is adding an [HTTP connector](https://learn.microsoft.com/en-us/fabric/real-time-intelligence/event-streams/overview) for **ingestion into** Eventstream. That direction is the opposite of Ghost inbound — use it when Ghost exports audit data **to** Fabric, not for Eventstream → Ghost.

## curl test (localhost)

```bash
SECRET="<from Settings → Generate webhook secret>"
curl -sS -X POST "http://127.0.0.1:8787/fabric/webhook" \
  -H "Content-Type: application/json" \
  -H "X-Ghost-Webhook-Secret: $SECRET" \
  --data @docs/samples/fabric-eventstream/ghost-native.json
```

## Related docs

- `docs/fabric-integration.md` — Fabric grant + export
- `docs/mcp-integration.md` — HTTP server + TLS
- `docs/command-registry.md` — `fabric_set_webhook_secret`, `fabric_webhook_status`
