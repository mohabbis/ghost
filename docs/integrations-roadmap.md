# Integrations Roadmap

What "connect Ghost to your stack" means, what exists today, and what's still a plan.

## Why this exists

Ghost's edge isn't that it's local-only — it's that every mutation goes through
`Intent -> Plan -> Policy check -> User approval -> Execution -> Audit log -> Undo`.
That pipeline is the thing competitors skip when they wire an agent straight
into your BI stack or your inbox. Being reachable from Microsoft Fabric/Power
BI, Google Cloud, and AI coding assistants (Claude, Cursor, Codex, ChatGPT) is
a distribution and stickiness advantage — it's just not a reason to weaken the
approval/audit/undo boundary that makes Ghost trustworthy in the first place.
Every integration below is additive to that pipeline, never a bypass of it.

## What exists today

- **Account sign-in** (`commands/account.rs`, `identity/oauth/`): "Sign in with
  Microsoft" / "Sign in with Google" via OAuth 2.0 + PKCE. This establishes
  *identity* — who the user is — so a later integration has someone to ask
  for access on behalf of. It is not, by itself, a data-access grant to
  anything: no Fabric workspace, no Google Cloud project, and no AI-assistant
  session is reachable just because a user signed in. See
  `docs/microsoft-auth.md`, `docs/core-boundaries.md`, and
  `docs/command-registry.md`.
- **Identity / grant separation** (`identity/`): `AccountIdentity`,
  `IntegrationGrant`, and encrypted `TokenRecord` are stored separately.
  Signing in creates an `Identity` grant only; Fabric/Power BI require their
  own grants (`integrations/microsoft/`). Legacy `account.json` migrates to
  `identity.json` automatically.
- **Power BI audit export** (`integrations/microsoft/power_bi/`,
  `commands/integrations.rs`, gated behind `--features experimental`): the
  first working Layer B connector. Requires a separate, revocable
  `MicrosoftPowerBi` grant on top of base sign-in (`power_bi_request_grant`,
  incremental consent via the same OAuth flow, no second login); a pure,
  read-only preview of exactly what would be sent
  (`power_bi_export_preview`); and a push
  (`power_bi_push_audit_export`) that re-derives the payload from local
  Organizer execution history server-side — never trusting a frontend-
  supplied snapshot — and PII-masks every string field before sending. v1
  always targets the signed-in user's own "My workspace," with no
  workspace/dataset picker yet. See `docs/power-bi-integration.md`.
- **Fabric connector (partial)** (`integrations/microsoft/fabric/`,
  `commands/integrations.rs`, experimental): separate `MicrosoftFabric`
  grant, workspace listing (`fabric_list_workspaces`), lakehouse listing
  (`fabric_list_lakehouses`), export preview (`fabric_export_preview`), and
  OneLake push (`fabric_push_audit_export`). Settings UI uses workspace/lakehouse
  dropdown pickers. Inbound intent queue (`fabric_list_inbound_intents`,
  `fabric_record_inbound_intent`, `fabric_dismiss_inbound_intent`) surfaces
  external signals without auto-executing. **Webhook ingestion**
  (`POST /fabric/webhook`, `fabric_set_webhook_secret`) records intents when the
  HTTP server is running. See `docs/fabric-integration.md`.
- **Google Cloud Storage export** (`integrations/google/`,
  `commands/integrations.rs`, experimental): separate `GoogleCloud` grant,
  bucket listing (`google_list_buckets`), per-bucket grant binding
  (`google_bind_export_bucket`), export preview, and GCS push
  (`google_push_audit_export`). See `docs/google-cloud-integration.md`.
- **MCP HTTP server** (`mcp/http.rs`, `ghost mcp serve http [port]` or
  `mcp_start_http_server`): localhost by default; optional LAN bind with bearer
  auth; optional in-process TLS (PEM cert/key); shared listener for `POST /mcp`
  and `POST /fabric/webhook`. **Cloud relay** (`mcp/relay.rs`, reference
  `mcp_relay_server` bin): outbound HTTPS wide-area routing — `docs/mcp-relay.md`.
  **Fabric Eventstream bridges**: `docs/fabric-eventstream-webhook.md` + samples.
- **Organizer TOCTOU + boundary hardening** (`organizer/file_identity.rs`,
  `policy/boundary.rs`, `executor.rs`): scan-time dev/ino (or Windows NTFS
  timestamps) re-checked at execution; canonical path zone-boundary re-check after
  `canonicalize`; symlink sources refused.
- **Local intelligence providers** (`intelligence/local/`, experimental):
  Ollama, LM Studio, and OpenAI-compatible localhost adapters;
  `intelligence_discover_local` probes localhost only. Settings UI and
  `organizer_intelligence_suggest` (Organizer AI suggestions button) are
  built. See `docs/local-models.md`.
- **Local MCP stdio server** (`mcp/server.rs`, `main.rs` `ghost mcp serve`):
  JSON-RPC tools for status, Zone listing, plan creation/validation,
  approval request/status (`ghost.request_approval`, `ghost.get_approval_status`),
  signed-token-gated execution, undo, and optional pairing. See
  `docs/mcp-integration.md`.
- **Integration module boundaries** (`integrations/`, `intelligence/`, `mcp/`):
  Layer B business connectors (Power BI + partial Fabric), Layer C internal
  providers (disabled by default, gated behind `--features experimental`),
  and MCP server — see `docs/ai-provider-architecture.md` and
  `docs/integration-threat-model.md`.
- **Ghost's own MCP-planning surface** (`docs/mcp-integration.md`): the
  existing model for letting an external AI client (an "AI-assistant
  connector," in the language below) drive Ghost — status, Zone listing,
  scans, plan creation, approval requests, execution of already-approved
  plans, and undo, all without exposing raw filesystem authority to the
  model. This is the template every stack integration below should follow:
  the model proposes, Ghost plans and gates, the user approves, Ghost
  executes and audits.

Everything else in this document is **planned or partially built** — stock
builds do not register intelligence, Fabric, Power BI, or MCP commands unless
compiled with `--features experimental`. Do not describe experimental
integrations as shipped in default product copy.

## Setting up sign-in for local development

Ghost ships with no client IDs of its own — "Sign in with Microsoft"/"Sign in
with Google" is unavailable until an operator registers a public-client app
registration and supplies a client ID (PKCE public clients don't have or need
a client secret):

- **Microsoft**: register an app in Entra ID (Azure Portal → App
  registrations), set it up as a public client with a redirect URI of
  `http://127.0.0.1/callback` (the loopback port is chosen at runtime, so the
  provider must accept any port — Microsoft's "mobile and desktop
  applications" platform type does this), and grant the `openid email
  profile offline_access` delegated scopes.
- **Google**: create an OAuth client of type "Desktop app" in Google Cloud
  Console, which accepts a loopback redirect the same way.

Then either set `integrations.microsoft_client_id` / `integrations.google_client_id`
in Ghost's config (`get_config`/`update_config`), or export
`GHOST_MS_CLIENT_ID` / `GHOST_GOOGLE_CLIENT_ID` for local runs — the env var
is dev-only and does not persist to disk. See `core/oauth.rs` for the exact
scopes and endpoints used.

## Microsoft stack (Fabric / Power BI)

1. **Identity**: reuse the Microsoft OAuth link from account sign-in via a
   separate incremental-consent grant per integration (`identity::run_grant_flow`)
   — never a second login. **Built** for Power BI
   (`IntegrationKind::MicrosoftPowerBi`); Fabric's grant type
   (`IntegrationKind::MicrosoftFabric`) exists in the type system but its
   exact scopes are set (`integrations/microsoft/scopes.rs` —
   `https://api.fabric.microsoft.com/.default`).
2. **Read surface first**: exporting Organizer audit history / execution
   summaries into a Fabric workspace or a Power BI dataset (e.g. "how much
   filing time did Ghost save this month") is a safe first cut — it's Ghost
   sending its own audit data outward, not Fabric reaching in to trigger
   mutations. **Built for Power BI** (`power_bi_export_preview` +
   `power_bi_push_audit_export`, gated behind `--features experimental`).
   **Partial for Fabric** — grant, workspace/lakehouse list, export preview,
   and OneLake push (`fabric_push_audit_export`) are built.
3. **Write surface later, if ever, stays gated**: anything that would let a
   Fabric/Power BI trigger *cause* Ghost to move files or run a routine has
   to enter through the same `Intent -> Plan -> Policy check -> User approval`
   chain as a locally-initiated action — no service-to-service trigger skips
   the approval screen. Nothing like this exists yet, for either Fabric or
   Power BI.

## Planned: Google Cloud

Same shape as Microsoft: reuse the Google OAuth identity, start with
Ghost-initiated reads/exports (e.g. writing an audit export to a bucket the
user already owns), and treat any inbound trigger as subject to full policy
review before it can propose a mutation.

## Planned: AI-assistant connectors (Claude, Cursor, Codex, ChatGPT)

These are different in kind from Fabric/Google Cloud: they're coding/agent
tools, not data platforms, so the integration point is "let the assistant
call Ghost's MCP planning surface" rather than "sync data with Ghost."
Concretely, this means exposing the same status/plan/approve/execute/undo
surface described in `docs/mcp-integration.md` to more MCP-capable clients,
not building bespoke per-vendor code paths. The model proposes a plan; it
never gets standing authority to execute one unapproved.

## What must never change because of this roadmap

- No integration may execute a mutating action without going through
  `policy::evaluate` and an explicit user approval, the same as every other
  Organizer/Routines action.
- No integration may read more than it needs — scope requests narrowly
  (a specific workspace/bucket/dataset), never "everything in the account."
- Signing in does not imply consent to any specific integration; each
  integration (Fabric export, Google Cloud export, an MCP connector) is a
  separate, visible, revocable grant.
- Ghost still keeps local audit and undo data on-device regardless of which
  integrations are enabled — an integration can add a destination for a
  copy of that data, it cannot replace the local trail.
