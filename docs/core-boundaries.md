# Core Boundaries

Product and engineering boundaries for Ghost.

This document defines what belongs in the trusted core and what must remain experimental. `AGENTS.md` is the canonical AI-agent contract; this file explains the product boundary those agents must preserve.

## Product loop

Ghost is built around this loop:

```text
Record -> Inspect -> Approve -> Replay -> Audit -> Undo
```

For meaningful operations, use the full trust pipeline:

```text
Intent -> Plan -> Policy check -> User approval -> Execution -> Audit log -> Undo path
```

If a feature cannot fit this pipeline, it does not belong in the trusted core. Future AI-client and MCP integrations must preserve this split: models produce structured intent or suggestions; Ghost plans, verifies, obtains approval, executes deterministically, audits, and records undo data.

## Stable core

Stable core capabilities may define the product contract.

Allowed stable areas:

- permission checks and permission requests (including app relaunch so macOS applies a fresh grant);
- explicit user-approved recording;
- replay with cancellation, pause, resume, and playback speed controls;
- workflow save/load/list/delete;
- workflow event schema and migrations;
- element inspection and recorded-event review;
- local authentication and at-rest workflow protection;
- account sign-in via Microsoft/Google OAuth 2.0 + PKCE (`commands/account.rs`, `identity/oauth/`) as an identity link, not a data-access grant — `AccountIdentity` is separate from `IntegrationGrant` and encrypted `TokenRecord`; signing in creates identity-only consent, not Fabric/Power BI access (see `docs/microsoft-auth.md`, `docs/integrations-roadmap.md`);
- diagnostics and safe telemetry export;
- policy checks and command risk classification;
- audit log primitives;
- undo journal primitives;
- Ghost Organizer scan/plan/review/execute flow;
- provider-neutral MCP planning surfaces that expose status, Zone listing, scans, plan creation, validation, explanations, approval requests, execution of already approved plans, run verification, and undo without exposing raw filesystem authority;
- enterprise financial-operations domain models and playbook/runtime primitives when they are commandless scaffolding or otherwise preserve the trust pipeline;
- on-device OCR of user-supplied images (`run_ocr_on_image`, macOS Vision / Windows OCR) and deterministic parsing of that OCR'd text into structured ID-document fields (`parse_id_document` / `core/id_scan.rs`). OCR runs only on images the user hands in and never touches the network; the parser is pure text-in/struct-out (no image, IO, or capture). Any resulting personal fields stay local — never uploaded, never used to auto-execute anything.

Stable core behavior must be:

- user-started or user-approved;
- local-first;
- scoped to known folders, apps, windows, domains, or actions;
- testable without relying on AI output;
- interruptible where execution takes time;
- auditable when it changes meaningful state;
- reversible where practical.

## Experimental surfaces

These areas may exist for research or developer mode, but they must not be treated as production-ready product boundaries:

- AI workflow analysis;
- AI workflow optimization;
- AI workflow generation from prompts;
- proactive observer mode;
- learned-pattern suggestions;
- cloud sync and workspaces;
- enterprise audit logs;
- analytics dashboards;
- visual regression checks;
- data-source-driven workflow testing;
- broad proactive intelligence;
- remote MCP endpoints or network-accessible agent integrations;
- internal AI provider adapters until they are suggestion-only, scoped, and explicitly configured;
- secure tunneling, plugin SDKs, workflow marketplaces, and signed approval-token infrastructure until their authentication, authorization, policy, audit, and undo boundaries are implemented and tested;
- enterprise financial connectors that can approve payments, post journal entries, transmit funds, modify closed periods, or decide check-cashing outcomes.

Experimental work must be:

- compiled out of the stock Tauri IPC registry unless `--features experimental` is used;
- feature-gated or clearly labeled;
- isolated from trusted execution;
- denied by default for risky mutations;
- excluded from default UI unless explicitly requested;
- documented with limits and failure modes before promotion.

## Integration boundary

Use one Ghost MCP server for external AI-client interoperability rather than separate vendor-specific integrations. Local read-only MCP tools may be promoted only when they preserve Ghost's privacy defaults, expose metadata instead of content by default, and cannot mutate state. Planning tools must produce reviewable plans; execution tools must require approval from the Ghost desktop UI and must verify a short-lived, single-use token bound to the exact plan hash. See `docs/mcp-integration.md`.

AI provider adapters for Ghost's own UI are separate from MCP. Provider output is suggestion-only and must flow through redaction, deterministic planning, policy evaluation, desktop approval, execution, audit, and undo. See `docs/ai-provider-boundaries.md`.

## Deny-by-default actions

The following actions must not execute silently:

- deleting files;
- overwriting files;
- moving files outside approved folders/Zones;
- uploading files;
- sending messages;
- submitting forms;
- typing into unknown apps or fields;
- running shell commands;
- exposing raw filesystem APIs to AI clients or plugins;
- letting AI clients approve plans, modify approved plans, reuse approvals, or escalate permissions;
- capturing screenshots or screen contents;
- reading email, browser, or document contents;
- using network/cloud sync;
- replaying actions outside approved app/window/folder scope.

These actions require policy checks, explicit approval, and audit behavior. Some should remain unavailable until the policy layer is real.

## Command-surface policy

New Tauri commands must be assigned to a module before registration:

1. Stable core
2. Auth and protection
3. Diagnostics
4. Experimental

Every command needs a risk class. Use the classes in `AGENTS.md` and `docs/command-registry.md`.

Stable commands should stay boring, explicit, and testable. Experimental commands may move quickly, but they must remain separate from the trusted product surface.

## Schema policy

Workflow files should carry a schema version. Breaking changes need explicit migration code.

Recommended envelope:

```json
{
  "schema_version": "0.2.0",
  "app_version": "1.0.12",
  "created_at": "2026-06-23T00:00:00Z",
  "platform": "macos",
  "steps": [],
  "safety": {
    "requires_confirmation": true,
    "allowed_apps": [],
    "blocked_apps": [],
    "allowed_folders": [],
    "blocked_folders": []
  }
}
```

Schema changes should include:

- migration path;
- fixture updates;
- invalid-input behavior;
- compatibility notes if old workflows may fail.

## Promotion gate

A feature can move from experimental to stable only when it has:

1. clear user-facing behavior;
2. explicit scope boundaries;
3. documented failure modes;
4. tests for valid, invalid, denied, and interrupted flows where relevant;
5. policy checks for risky actions;
6. approval before meaningful mutation;
7. audit logging where state changes;
8. undo support where practical;
9. privacy review for sensitive reads;
10. docs updated in `AGENTS.md`, `docs/command-registry.md`, and user-facing docs if relevant.

## Release-readiness gate

Ghost should not be described as user-ready until these are true:

- macOS release is Developer ID signed and notarized;
- Windows release is signed;
- at-rest protection uses only approved encryption paths;
- replay reliability is tested across real native and browser workflows;
- experimental commands are feature-gated or clearly labeled;
- app UI and marketing/download site are separated or generated from one source;
- workflow schema versioning and migration tests exist;
- Organizer preview/approval/audit/undo behavior is implemented and tested.

## Agent checklist

Before finishing any feature or refactor, verify:

- the work preserves the product loop;
- risky operations are denied by default;
- stable and experimental surfaces remain separated;
- user-facing copy does not overpromise;
- docs reflect any boundary change;
- validation was run or the gap is reported.
