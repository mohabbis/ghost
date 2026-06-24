# Command Registry

Ghost exposes Tauri commands through `src-tauri/src/commands.rs`, but command implementations are split by product boundary.

## Modules

| Module | Purpose |
|---|---|
| `commands/core.rs` | Stable local automation surface: recording, replay, inspection, workflow storage, and permissions. |
| `commands/auth.rs` | Local password state and at-rest workflow protection controls. |
| `commands/diagnostics.rs` | Config, telemetry export, and performance summaries. |
| `commands/experimental.rs` | AI, observer mode, cloud sync, analytics, visual checks, data sources, and reliability experiments. |

## Policy

New commands should not be added directly to `commands.rs`. Put the implementation in the right module, then re-export it through the registry.

Stable core commands should remain boring, explicit, and testable. Experimental commands can move quickly, but they should stay visibly separated until they have documented reliability and UX limits. Yes, this is bureaucracy, but it is the useful kind, not the kind that makes you upload the same PDF six times.

## Naming

Keep existing Tauri command names stable unless there is a migration plan. Frontend calls depend on these names, and breaking them silently is how apps become haunted.

## Promotion path

An experimental command can move toward the stable core only after:

1. It has clear user-facing behavior.
2. It has failure modes documented.
3. It has tests for valid, invalid, and interrupted flows.
4. It does not weaken local privacy or workflow safety.
5. It is reflected in `docs/core-boundaries.md`.
