# Command Registry

Operational rules for Tauri commands in Ghost.

Read this before adding, moving, renaming, or exposing commands.

## Canonical agent contract

`AGENTS.md` is the source of truth for AI agent behavior. This file applies that contract to the command surface.

Every meaningful command should support the product loop:

```text
Intent -> Plan -> Policy check -> User approval -> Execution -> Audit log -> Undo path
```

## Registry rule

`src-tauri/src/commands.rs` should stay a thin registry/re-export surface.

Do not place large command implementations directly in `commands.rs`.

Use the right module, then re-export/register from the registry layer.

## Modules

| Module | Purpose | Allowed shape |
|---|---|---|
| `commands/core.rs` | Stable local automation: permissions, recording, replay, inspection, workflow storage | explicit, tested, user-started |
| `commands/auth.rs` | Local password state and at-rest workflow protection | local-only, no network dependency |
| `commands/diagnostics.rs` | Config summaries, telemetry export, performance/debug data | read-first, redacted, user-initiated export |
| `commands/experimental.rs` | AI, observer mode, cloud sync, analytics, visual checks, data sources, research features | gated, labeled, not default product UI |

## Required command metadata

When adding or changing a command, document these in code comments or nearby docs:

- command name;
- module;
- risk class;
- whether it reads user content;
- whether it mutates local state;
- whether it touches network/remote state;
- whether approval is required;
- whether audit logging is required;
- whether undo is possible;
- whether the command is stable or experimental.

## Risk classes

| Class | Meaning | Examples | Requirement |
|---|---|---|---|
| `safe-read` | Reads non-sensitive local metadata | list workflows, read settings | normal user action |
| `sensitive-read` | Reads user content or private context | file contents, selected text, window titles, screenshots | scoped permission and visible state |
| `local-mutate` | Changes local state | move file, rename file, delete workflow | plan, approval, audit, undo when possible |
| `external-mutate` | Sends data or changes remote state | upload, send message, submit form, sync | deny by default unless explicitly scoped |
| `os-control` | Controls input or other apps | click replay, typing, shell command | approved target scope and emergency stop |
| `experimental` | Not trusted product behavior yet | AI-generated workflows, observer suggestions | gated, labeled, isolated |

If a command fits more than one class, use the highest-risk class.

## Stable command requirements

A command can be part of the stable core only when it has:

1. clear user-facing behavior;
2. scoped inputs;
3. documented failure modes;
4. validation for invalid input;
5. interruption/cancel behavior where applicable;
6. audit behavior for meaningful operations;
7. undo support for reversible mutations where practical;
8. tests or a documented validation path.

## Experimental command rules

Experimental commands may exist for development, but they must:

- live in `commands/experimental.rs` or another clearly experimental module;
- be hidden from default product UI unless explicitly requested;
- avoid direct mutation unless routed through policy and approval;
- avoid cloud/network behavior unless clearly scoped;
- include limits and failure modes before promotion.

## Naming

Keep existing Tauri command names stable unless there is a migration plan.

When renaming a command:

1. update frontend calls;
2. update command registration;
3. update tests or fixtures;
4. update docs;
5. leave compatibility wrappers only when needed.

## Promotion path

An experimental command can move toward the stable core only after:

1. it has clear user-facing behavior;
2. it has failure modes documented;
3. it has tests for valid, invalid, denied, and interrupted flows where relevant;
4. it does not weaken local privacy;
5. it does not bypass approval for meaningful mutation;
6. it writes audit/undo data where required;
7. it is reflected in `docs/core-boundaries.md` and `AGENTS.md` if the product contract changes.

## Agent checklist

Before finishing command work, verify:

- the command is in the right module;
- the risk class is clear;
- risky operations go through policy;
- user approval is required where needed;
- audit and undo behavior are addressed;
- experimental work is gated or labeled;
- docs were updated with the behavior change;
- checks were run or the validation gap is reported.
