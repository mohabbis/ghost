# Ghost

[![Download](https://img.shields.io/badge/Download-Latest-8d7bff?style=flat-square)](https://github.com/mohabbis/ghost/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![Release](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/release.yml?style=flat-square&label=Release)](https://github.com/mohabbis/ghost/actions/workflows/release.yml)
[![macOS](https://img.shields.io/badge/macOS-12%2B-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10%2F11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

> **Desktop automation you approve before it acts.**

Ghost is a local-first desktop automation product for macOS and Windows. It turns repeated file and desktop work into reviewable routines: scan or record, inspect the plan, approve the exact actions, execute inside policy boundaries, audit every change, and undo when needed. Long term, Ghost is the trusted execution layer for AI-assisted local file operations: AI can reason and suggest, but Ghost verifies, the user approves, and deterministic code executes.

Ghost is not a cloud agent and not a silent macro recorder. The product is built around a simple constraint:

```text
Record -> Inspect -> Approve -> Replay -> Audit -> Undo
```

## Why Ghost Exists

Software teams, automation engineers, practice admins, students, and small operations teams spend hours cleaning up downloads, filing test reports and build logs, renaming client documents, filing statements, and repeating the same desktop chores. That work is repetitive enough to automate, but too sensitive for black-box automation.

Common tools miss the shape of the problem:

| Tool | Why it falls short |
| --- | --- |
| Macro recorders | Replay blind coordinates and break when the UI moves. |
| Cloud automation | Works well for APIs, not local folders and desktop apps. |
| Enterprise RPA | Powerful, expensive, and too heavy for weekly SMB workflows. |
| AI agents | Hard to inspect, hard to bound, and risky around client files. |
| Scripts | Useful for engineers, brittle for non-technical users. |

Ghost focuses on the missing middle: local workflows that need preview, approval, audit logs, and recovery.

## Start Here: Ghost Organizer

The current wedge is **Ghost Organizer**, a safe file-organization flow for test reports, coverage exports, build logs, screenshots, invoices, statements, and messy download folders.

```text
Select folder -> Scan -> Propose plan -> Review -> Approve -> Move/Rename -> Audit -> Undo
```

What makes it different:

- **Preview before mutation**: every move and rename is shown before it runs.
- **Deny by default**: Ghost only works inside Zones and capabilities you choose.
- **No silent deletes or overwrites**: conflicts are held for review.
- **Undo-first execution**: reversible operations write undo data before changing files.
- **Local audit trail**: each run records what happened, why it was allowed, and how to recover.

## Five-Minute Demo Flow

1. Open Ghost Organizer.
2. Select a folder such as `~/Downloads`.
3. Scan the folder. Nothing changes during scan.
4. Review the proposed plan: created folders, moves, renames, conflicts, denied actions.
5. Approve the plan.
6. Ghost executes the approved actions and writes an audit log plus undo journal.
7. Use Undo to restore the original state when needed.

That is the product: boring, inspectable, and reversible.

## Trust Model

Ghost enforces the same pipeline for meaningful operations:

```text
Intent -> Plan -> Policy check -> User approval -> Execution -> Audit log -> Undo path
```

| Stage | Purpose |
| --- | --- |
| Intent | Capture what the user is trying to do. |
| Plan | Produce exact proposed actions before mutation. |
| Policy check | Classify risk, scope, conflicts, and denied operations. |
| User approval | Require explicit approval for the final plan. |
| Execution | Run deterministic code, not raw model output. |
| Audit log | Persist what changed and why it was allowed. |
| Undo path | Store recovery data where reversal is possible. |

The hard boundary is intentional: experimental features may suggest; only the trusted core may mutate. This boundary also applies to future MCP and AI-client integrations: clients may request scans, plans, explanations, approvals, execution of approved plans, and undo, but they must never receive raw filesystem execution authority.

## Current Status

Ghost is an early technical preview. Treat it as useful, inspectable software under active development, not a production-grade unattended automation platform.

Working now:

- Ghost Organizer scan, preview, approval, execution, audit, and undo surfaces.
- Zones and capabilities for local folder boundaries.
- Ghost Guard policy checks for risky workflows.
- Deterministic event compression for readable replay timelines.
- Local workflow storage and run history.
- macOS and Windows desktop packaging paths.

Still being hardened:

- Release signing and notarization.
- Cross-app replay reliability where semantic targeting falls back to coordinates.
- Richer target resolution for windows, controls, and app-specific UI.
- Installer and update polish.

## Download

| Platform | Download |
| --- | --- |
| macOS 12+ (Apple Silicon + Intel) | [Ghost.dmg](https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg) |
| Windows 10 / 11 (64-bit) | [Ghost_Setup.exe](https://github.com/mohabbis/ghost/releases/latest/download/Ghost_Setup.exe) |

Current builds are preview quality. The current app version is `v1.2.7`. macOS builds may be ad-hoc signed; if macOS blocks the app, approve it under **System Settings -> Privacy & Security**. Verify downloads with [`SHA256SUMS.txt`](https://github.com/mohabbis/ghost/releases/latest/download/SHA256SUMS.txt). Notarized releases require Apple signing secrets — see `RELEASING.md`.

Ghost ships a signed, user-approved auto-updater: on launch it checks for a newer release and, if one exists, offers it — it installs only after you click **Update now**, and never swaps itself out silently. See [`docs/auto-update.md`](docs/auto-update.md).

## Architecture

Ghost is a Tauri 2 desktop app with a Rust backend and a vanilla HTML/CSS/JS frontend.

| Area | Responsibility |
| --- | --- |
| `src-tauri/src/organizer/` | File scanning, planning, execution, audit, and undo. |
| `src-tauri/src/policy/` | Deny-by-default capability checks and risk boundaries. |
| `src-tauri/src/core/` | Compression, guard review, dry-run, workflow handling. |
| `src-tauri/src/platform/` | macOS and Windows capture/replay backends. |
| `src/` | Desktop app UI packaged by Tauri. |
| `public/` | Hosted product/marketing site. |
| `docs/` | Operational specs, command registry, roadmap, MCP boundaries, and security notes. |

The command surface is intentionally classified by risk. Before changing Tauri commands, read [`AGENTS.md`](AGENTS.md), [`docs/command-registry.md`](docs/command-registry.md), and [`docs/core-boundaries.md`](docs/core-boundaries.md). For AI-client interoperability, use one MCP boundary instead of vendor-specific integrations; see [`docs/mcp-integration.md`](docs/mcp-integration.md) and [`docs/ai-provider-boundaries.md`](docs/ai-provider-boundaries.md). For the longer-term execution architecture those boundaries are heading toward, see [`docs/next-generation-architecture.md`](docs/next-generation-architecture.md).

## Development

```bash
# Install Rust and Tauri CLI
cargo install tauri-cli --version "^2.0" --locked

# Run the main gates
make ci
make check
make test

# Run or build the desktop app
make dev
make build
```

Equivalent direct commands:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo tauri build --no-bundle
```

## Privacy Defaults

- No camera.
- No microphone.
- No hidden screen capture.
- No background email, browser, or document reading.
- No cloud-first storage for workflow/organizer data.
- No unapproved network calls beyond account sign-in and explicitly-approved stack integrations.
- Keyboard and pointer capture only during explicit recording or approved replay.

Workflow data, logs, settings, audit history, and undo journals stay local and encrypted at rest by default, regardless of which account or stack integrations are enabled. Ghost supports account sign-in (Microsoft/Google) and is built to connect with the tech stacks users already run — Microsoft Fabric/Power BI, Google Cloud, and AI-assistant connectors (Claude, Cursor, Codex, ChatGPT) — without weakening the trust pipeline above. See `docs/integrations-roadmap.md` for what's built versus planned.

## Roadmap

1. Keep CI green and command surfaces classified.
2. Harden Ghost Organizer planning, review, execution, audit, and undo.
3. Improve target resolution and replay reliability.
4. Finish release signing and installer quality.
5. Add suggestion-only intelligence behind clear gates after the trusted core is reliable.
6. Add a local MCP server as a single provider-neutral integration surface for read, scan, plan, validate, explain, approval, execution, and undo workflows.
7. Add signed approval tokens, remote pairing/relay, provider abstraction, and plugin/workflow capabilities without weakening local-first execution ownership.

## Contributing

Good contributions make Ghost more predictable:

- Organizer correctness and edge-case handling.
- Policy and security hardening.
- Replay reliability and target resolution.
- Accessibility and UX improvements that preserve explicit approval.
- Clear docs, examples, and tests.

Avoid contributions that move Ghost toward unbounded autonomy, hidden observation, cloud dependency, or silent mutation. User approval is not a speed bump; it is the product.

## FAQ

**Does my data leave my machine?**

No by default. Workflows, logs, settings, audit history, and undo journals are local.

**Can Ghost delete files?**

Ghost Organizer blocks silent deletion. Mutations are previewed, approved, audited, and reversible where possible.

**Can AI execute actions?**

No. AI or heuristics may suggest plans in gated surfaces, but deterministic Ghost code executes only approved plans.

**Can Ghost run while I am away?**

Not as a default product behavior. Ghost is designed for explicit, reviewable, interruptible work.

**Why not use a macro recorder?**

Macro recorders replay coordinates. Ghost is built around inspection, policy checks, semantic targeting, audit logs, and undo.

## License

MIT - see [LICENSE](LICENSE).
