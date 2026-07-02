# Ghost — Implementation Status

Honest accounting of what is built, what is prototype, and what is disabled.
This file describes the code as it is; it is not a roadmap (see
`docs/PRODUCT_ROADMAP.md`) and not a vision document. If a claim here drifts
from the code, the code wins — fix this file.

## Feature status matrix

| Feature | Status | Notes |
|---|---|---|
| Record / replay (macOS, Windows) | ✅ Available | CGEventTap / Win32 hooks capture; enigo replay with press/release pairing, timestamp pacing, pause/cancel, speed control |
| Replay history | ✅ Available | `ExecutionRecord` per run (`core/execution.rs`), surfaced via `get_replay_history` and a frontend history view |
| Workflow save/load/delete | ✅ Available | Local JSON, path-sanitized, optional at-rest encryption via local password (`auth.rs`) |
| Event compression review timeline | ✅ Available | Deterministic `InputEvent` → `CompressedStep` compression (`core/compression/`), `compress_workflow` command, timeline UI |
| Ghost Organizer (plan/execute/undo) | ✅ Available | Read-only planner, policy-checked executor, append-only audit log, undo journal; never silently deletes or overwrites |
| Policy engine + Zones | ✅ Available | Deny-by-default evaluation (`policy/`), SQLite-backed Zones and folder rules (`storage/`) |
| Local auth / workflow encryption | ✅ Available | Local password, at-rest envelope encryption; no accounts, no server |
| Semantic element targeting | ✅ Available | AX/UIA descriptor capture with coordinate fallback and nearby re-resolution (self-heal spiral) |
| Reliable replay (retry/backoff) | 🧪 Experimental | Element-lookup retries with backoff; command registered only with `--features experimental` |
| AI workflow generation/analysis | 🟡 Prototype, gated | Suggestion-only; LLM calls behind `--features experimental`; deterministic code executes only approved plans |
| Observer mode | 🟡 Prototype, gated | In-memory `KnowledgeBase`; heuristic pattern detection; no persistence; `--features experimental` only |
| Visual regression checks | 🟡 Prototype, gated | Screenshot capture + SSIM comparison on macOS/Windows; `--features experimental` only |
| Data-driven testing | 🟡 Prototype, gated | CSV/JSON/env variable sources, first-row parsing only; `--features experimental` only |
| Workspace / audit-log surface | 🟡 Prototype, gated | In-memory only (`core/cloud.rs`); not durable, not multi-user; do **not** pitch as enterprise audit logging. The Organizer's audit log (`audit/`) is the real, durable one |
| Cloud sync | 🚫 Disabled | `authenticate`/`sync_workflows`/`load_workflows` hard-error ("Cloud sync is not available in this build"); no network backend exists |
| OCR element selectors | 🚫 Not implemented | `ElementSelector::OCR` resolves to an explicit error (`core/wait.rs`); not on the near-term roadmap |

Everything marked "gated" is compiled out of default builds: the
`experimental` Cargo feature is off by default, so a stock build does not
register or expose those commands. The frontend hides the experimental panel
unless `is_experimental_enabled` reports the feature is on.

## Architecture

```text
src/                    # Tauri frontend (vanilla JS/HTML/CSS, no bundler)
src-tauri/src/
  lib.rs                # app wiring + generate_handler! registry
  engine.rs             # recording/replay orchestration
  commands/             # IPC surface split by boundary (core, auth,
                        #   compression, diagnostics, organizer, updates,
                        #   experimental [feature-gated])
  core/                 # events, traits, replay_support, execution history,
                        #   compression, wait, guard, security; experimental-
                        #   facing ai/cloud/llm/vision/knowledge
  platform/             # macos.rs, windows.rs, headless.rs (Linux CI)
  policy/               # deny-by-default trust engine
  storage/              # SQLite migrations, zones, execution history
  organizer/            # scanner/classifier/naming/conflict/planner/executor/undo
  audit/                # append-only audit log + undo journal
```

Every meaningful operation flows through the trust pipeline:

```text
Intent -> Plan -> Policy check -> User approval -> Execution -> Audit log -> Undo path
```

## Where the details live

- Command surface and risk classes: `docs/command-registry.md`, `docs/core-boundaries.md`
- Organizer: `docs/organizer-planner.md`, `docs/organizer-executor.md`, `docs/organizer-commands.md`
- Policy engine: `docs/policy-engine.md`
- Compression: `docs/event-compression.md`, `docs/token-compression.md`
- Security posture: `SECURITY_AUDIT_SUMMARY.md`
- Agent contract: `AGENTS.md` (canonical), `CLAUDE.md`

## Validation

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Experimental code is not exercised by CI; run the same checks with
`--features experimental` locally when touching it.
