# Core Boundaries

Ghost is a local-first desktop automation product. The stable center of the product should stay small enough to trust, test, and explain without needing a séance.

## Stable core

These capabilities are allowed to define the product contract:

- permission checks and permission requests
- explicit user-approved recording
- replay with cancellation, pause, resume, and playback speed controls
- workflow save/load/list/delete
- workflow event schema and migrations
- element inspection and recorded-event review
- local authentication and at-rest workflow protection
- diagnostics and safe telemetry export
- Ghost Guard safety audit checks

Stable core work should prioritize boring reliability over novelty. For a desktop automation app, boring is not an insult. Boring is what keeps the app from clicking the wrong thing in the wrong window like a caffeinated gremlin.

## Experimental surfaces

These features are useful directionally, but should not be treated as production product boundaries yet:

- AI workflow analysis
- AI workflow optimization
- AI workflow generation from prompts
- proactive observer mode
- learned-pattern suggestions
- geek insights
- cloud sync and workspaces
- enterprise audit logs
- analytics dashboards
- visual regression checks
- data-source driven workflow testing

Until these are hardened, they should be described as experiments, hidden behind feature flags, or moved behind clearly named experimental command groups.

## Command-surface policy

New Tauri commands should be assigned to one of these groups before being registered:

1. Stable core
2. Auth and protection
3. Diagnostics
4. Experimental

Experimental commands may stay registered for frontend compatibility, but new UI should not present them as user-ready features until they have documented limits and reliability tests.

See [`command-registry.md`](command-registry.md) for the current module split and command inventory.

## Schema policy

Workflow files should carry a schema version. Breaking changes should include explicit migration code rather than hopeful parsing, the traditional software equivalent of closing your eyes while reversing a truck.

Current workflow schema version: `0.1.0`.

Event-only workflow files are saved with a top-level `schema_version`, `app_version`, `saved_at`, `name`, and `events` envelope. Legacy event-array files still load for backwards compatibility. Metadata workflow files carry `schema_version` and `app_version` directly on the workflow object, with serde defaults for older files that predate the fields.

Recommended envelope:

```json
{
  "schema_version": "0.1.0",
  "app_version": "1.0.12",
  "saved_at": 1782252000,
  "name": "Open Reports",
  "events": []
}
```

## Release-readiness gate

Ghost should not be described as user-ready until these are true:

- macOS release is Developer ID signed and notarized
- Windows release is signed
- at-rest protection uses only Argon2id plus AES-256-GCM paths
- replay reliability is tested across real native and browser workflows
- experimental commands are feature-gated or clearly labeled
- app UI and marketing/download site are separated or generated from one source
- workflow schema versioning and migration tests exist
