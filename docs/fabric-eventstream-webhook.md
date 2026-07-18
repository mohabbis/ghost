# Fabric Eventstream → Ghost Webhook Bridge

Ghost accepts inbound **intents only** at `POST /fabric/webhook` on the MCP HTTP server. Nothing auto-executes — intents surface in Organizer for scan, review, and approval. The same endpoint can carry Fabric events, batch-landed nudges, or a POS close/settlement bridge payload. For generic non-Fabric bridges, Ghost also accepts the same payloads and secret at `POST /inbound/webhook`; `/fabric/webhook` remains the canonical path shown in Settings and examples.

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
fabric inbound intent queue → Organizer pending-intent list
```

## Ghost webhook contract

```http
POST /fabric/webhook HTTP/1.1
Host: your-ghost-host:8787
X-Ghost-Webhook-Secret: <from fabric_set_webhook_secret>
Content-Type: application/json
```

`POST /inbound/webhook` is an alias for the same handler and secret when the event source is not specifically Fabric-branded.

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
  "source": "ops.pipeline",
  "summary": "Nightly export pipeline completed — review Organizer plan"
}
```

### POS close / settlement bridge

`docs/samples/fabric-eventstream/pos-shift-close.json`

```json
{
  "zone_id": "register-close-zone",
  "source": "pos.close",
  "summary": "Shift closed — file today's receipts"
}
```

This is still an **inbound intent only**. Ghost may open today's Organizer Zone plan after a desktop user clicks **Create plan**; it must never type into the POS or replay OS input from the webhook itself.

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
  "source": "hub.bridge",
  "summary": "Avg temperature 82.4 exceeded threshold in 5m window (device-7)"
}
```

## Bridge patterns

### Power Automate / Activator custom action

1. Create an Activator rule on Eventstream or Real-Time Hub events.
2. Action type: **Power Automate flow** with an **HTTP** action.
3. POST to `https://<ghost-host>/fabric/webhook` with the secret header (`/inbound/webhook` is equivalent for a generic bridge).
4. Body: Ghost native JSON or map Activator `triggerBody()` fields into `summary`.

See [Activator trigger Power Automate flows](https://learn.microsoft.com/en-us/fabric/real-time-intelligence/data-activator/activator-trigger-power-automate-flows).

### Azure Function (Event Grid → Eventstream pattern)

Microsoft documents CloudEvents → Eventstream via a decoder function when Eventstream cannot ingest CloudEvents directly ([example](https://github.com/sandervandevelde/WagoEventDecoderFunctionApp)). Use the same function (or a second HTTP action) to **also** POST a Ghost-native payload to `/fabric/webhook` (or `/inbound/webhook` if you want a vendor-neutral route name).

### POS close bridge (Square / Toast / custom middleware)

Do **not** add a POS SDK to Ghost for this slice. Let your existing bridge layer
(Node service, webhook relay, Zapier/Make, Azure Function, etc.) translate the
POS close/settlement event into Ghost-native JSON with `source: "pos.close"` and
an optional `zone_id`.

That bridge may tell Ghost "shift closed"; Ghost still requires a local user to
create the Organizer plan and approve any file mutations. No POS key injection,
no silent OS control, and no certification claim.

### Eventstream HTTP source (preview)

Fabric is adding an [HTTP connector](https://learn.microsoft.com/en-us/fabric/real-time-intelligence/event-streams/overview) for **ingestion into** Eventstream. That direction is the opposite of Ghost inbound — use it when Ghost exports audit data **to** Fabric, not for Eventstream → Ghost.

## curl test (localhost)

```bash
SECRET="<from Settings → Generate webhook secret>"
curl -sS -X POST "http://127.0.0.1:8787/fabric/webhook" \
  -H "Content-Type: application/json" \
  -H "X-Ghost-Webhook-Secret: $SECRET" \
  --data @docs/samples/fabric-eventstream/pos-shift-close.json
```

`/inbound/webhook` accepts the same request if you prefer a generic bridge URL.

## Related docs

- `docs/fabric-integration.md` — Fabric grant + export
- `docs/mcp-integration.md` — HTTP server + TLS
- `docs/command-registry.md` — `fabric_set_webhook_secret`, `fabric_webhook_status`
