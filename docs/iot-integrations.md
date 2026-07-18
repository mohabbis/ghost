# IoT integrations (feature-led)

Internet of Things ideas for Ghost only ship when they unlock a **user job**
Ghost already owns (file, verify, replay, audit) — not when they add a logo.

Devices and hubs may **propose**. Ghost still runs:

```text
Intent -> Plan -> Policy check -> User approval -> Execution -> Audit log -> Undo
```

Nothing in this doc is a shipped product claim. Stock Ghost does not bundle
scanner SDKs, POS drivers, or an MQTT broker. See `docs/integrations-roadmap.md`
for what is actually built (Fabric inbound webhook is the closest existing
signal path).

## Filter

Ship an IoT-backed idea only if all are true:

1. **Job** — removes a repeated desk/ops step (filing, closeout, verify).
2. **Delta** — better than “user opens Ghost and clicks Scan” alone.
3. **Trust** — device never auto-executes; Approve stays on the desktop.
4. **Honesty** — demoable without certification, always-on cam/mic, or silent
   OS control claims.

Connect-only ideas fail this filter and are listed under [Rejected](#rejected).

## Features

### F1 — Scanner Inbox (P0)

**Job:** Paper becomes files and a reviewable filing plan is waiting — the
bookkeeper does not hunt folders or remember to run Organizer.

**User-visible behavior:**

- Preset Zone “Scanner Inbox” on the MFP / ScanSnap drop folder.
- New files → inbound intent (or local “new files since last plan” signal).
- UI: badge / card “N new scans ready to file” with **Create plan** (not
  auto-move) → Approve → Organizer execute → audit / undo.

**Signal (thinnest path):** local scan-to-folder. No vendor SDK required for
v1 — Brother / Fujitsu / HP / Epson that drop PDF/JPEG into a directory are
enough.

**Current v1 scope:** Ghost ships a `Scanner Inbox` Organizer preset plus a
local ready-to-file banner driven by folder listing. Users still click
**Scan & Preview** manually; there is no background watcher, vendor SDK, or
auto-move path.

**Ghost modules:** Organizer Zones + planner/executor; Zone preset + docs;
optional inbound-intent card.

**Risk:** plan creation is read/plan; execute stays `local-mutate` with
approval. No camera permission.

**Demo one-liner:** Drop three invoices into the scan folder → Ghost shows a
plan → approve → files land under Finance with audit.

### F2 — Batch landed nudge (P0)

**Job:** When an export, SFTP drop, or nightly pipeline finishes, Ghost prompts
filing (or a saved-routine preview) instead of ignored chat noise.

**User-visible behavior:**

- Intent card: “Nightly AP export landed — review Zone …”
- Actions: **Create plan** | Dismiss. Never auto-execute.
- Later: deep-link to a named routine preview (still Approve).

**Signal:** ops webhook / Fabric Eventstream bridge → Ghost inbound intent
(`docs/fabric-eventstream-webhook.md`). Specialize on **document/batch
completion** events, not sensor novelty demos.

**Ghost modules:** Fabric (or future generic) inbound queue; Organizer banner;
experimental MCP HTTP listener for `POST /fabric/webhook`.

**Risk:** recording an intent is not mutation; convert = plan only.

**Demo one-liner:** Bridge posts “pipeline done” → Organizer card → Create
plan → approve.

### F3 — End-of-shift desk packet (P1)

**Job:** At register/POS close, Ghost assembles today’s filing plan (and
optional checklist routine) so closeout is not tribal knowledge.

**User-visible behavior:**

- Trigger: POS *close / settlement* webhook via a bridge (not Ghost inside the
  POS).
- Intent: “Shift closed — file today’s receipts.”
- Opens Organizer plan for the day’s drop Zone; optional routine checklist
  preview.

**Explicit non-goal:** Ghost must not type into the POS from a cloud event.
Guard Desk / POS Bridge remains a **prototype desk workflow — not certified
compliance** (`docs/audiences.md`, product audits).

**Delta vs F1:** time-bound “today’s batch” + closeout checklist, not every
scan.

### F4 — File-and-label (P2)

**Job:** After an approved Organizer run, optional folder/box label so physical
and digital filing match.

**User-visible behavior:**

- Post-run optional step: preview label text (path, period, client) → Approve
  print → DYMO / Brother QL / CUPS.
- Audit note: “label printed for run X.” Paper is not undoable — say so in UI.

**Ship only if** F1/F2 are habitual; otherwise it is a peripheral toy.

**Risk:** device I/O after explicit approval; treat as higher than pure FS
move; gate experimental.

### F5 — On-demand check/ID assist (P2, prototype-honest)

**Job:** User presents a check or ID image; Ghost OCR/parses into a review
packet; user decides next steps.

**User-visible behavior:** Existing on-device OCR + ID parse
(`run_ocr_on_image`, `parse_id_document`) and Guard Desk UI. IoT angle =
tethered imager as an **explicit user gesture** to supply bytes — never
ambient camera.

**Do not** market as KYC/AML/sanctions or compliance certification.

## Priority table

| Priority | Feature | Ghost surface | Mechanism | Status |
|---|---|---|---|---|
| P0 | F1 Scanner Inbox | Organizer | Scan-folder Zone preset + new-files → plan | Partial — `Scanner Inbox` preset + local ready-to-file banner; manual `Scan & Preview`, no watcher/vendor SDK |
| P0 | F2 Batch landed | Inbound intent → plan | Fabric/ops webhook bridge | Partial (Fabric intent queue, experimental) |
| P1 | F3 Shift close packet | Intent → Zone/routine preview | POS close webhook → bridge | Not built; marketing must stay qualified |
| P2 | F4 File-and-label | Post-run optional print | Label printer after Approve | Not built |
| P2 | F5 Check/ID assist | Guard Desk | User-supplied image OCR | Partial core OCR; desk workflow prototype |

## Intent contract (when a feature needs a signal)

Integrations that cannot use a plain folder watch should emit Ghost-native
JSON (same spirit as Fabric inbound):

```json
{
  "source": "scanner.folder|ops.pipeline|pos.close|hub.bridge",
  "summary": "12 new scans in Inbox",
  "zone_id": "optional-ghost-zone-uuid",
  "dedupe_key": "optional-stable-id",
  "payload_ref": "optional-path-or-uri-not-file-bytes"
}
```

Rules:

- No file bytes in the intent; paths only if already inside an allowed Zone.
- Statuses: `Pending` → `Converted` (user asked for a plan) / `Dismissed` /
  `Expired`.
- `Converted` means plan creation only; execute still requires Approve.
- Prefer hubs (Zapier, Make, Node-RED, Home Assistant, Azure Function) as
  **bridges** over first-party SDKs per gadget.

## Rejected

| Idea | Why it fails |
|---|---|
| Smart lights, thermostats, vacuums, speakers | No Ghost job |
| Always-on security cams, doorbells, microphones | Privacy non-negotiable |
| “Works with Azure IoT” badge without F2 cards | Connect-only |
| Cloud event → direct POS / OS input | Silent `os-control`; burns trust |
| Embed MQTT / IoT Hub client in Ghost core | Infra without a user job |
| Industrial PLC / SCADA control | Wrong product |
| Ambient home automation as a Ghost surface | Wrong audience and thesis |

## Implementation order (after this design)

1. **F1** — document + Zone preset “Scanner Inbox”; optional new-files intent
   card (folder watch before any scanner SDK).
2. **F2** — harden inbound intent UX (multi-intent list, Create plan =
   `Converted`); keep `/fabric/webhook`; generic alias only when needed.
3. **F3** — POS close → same intent queue; no POS key injection.
4. **F4 / F5** — only with clear habit and honest labeling.

Separate PRs per feature. Do not bundle with AI suggestions or marketplace
work.

## Related docs

- `docs/integrations-roadmap.md` — account / Fabric / GCS / MCP reality
- `docs/fabric-eventstream-webhook.md` — inbound intent bridge pattern
- `docs/organizer-planner.md` / `docs/organizer-executor.md` — trust pipeline
- `docs/core-boundaries.md` — stable vs experimental; OCR scope
- `docs/audiences.md` — what stays off the public surface
