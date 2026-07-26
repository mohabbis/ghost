# Ghost

[![Download](https://img.shields.io/badge/Download-v2.0.3-14b8a6?style=flat-square)](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![macOS](https://img.shields.io/badge/macOS-12+-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)
[![Windows](https://img.shields.io/badge/Windows-10/11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

> **Ghost is auditable, approval-gated automation for work that can't go to the cloud.**
> It scans, proposes an exact plan, waits for your approval, then acts — moving, renaming,
> or filling in — and seals a tamper-evident receipt with an undo path for every run,
> entirely on your machine.

Under the hood Ghost runs one trust pipeline — **Capture → Review → Approve → Execute →
Verify → Recover** — over both file operations and app/website steps. **Ghost Organizer**,
which safely cleans up messy local folders, is the flagship expression of that engine today.
The same pipeline is what lets Ghost grow into auditable workflows for accounting, legal, and
other work where a cloud tool is a nonstarter — without ever weakening the "nothing leaves
your machine" promise.

**Who it's for:** people accountable for recurring operational files and workflows who handle
client or financial data and have been told it can't go through a cloud tool — bookkeepers and
finance/admin staff, consultants handling client deliverables, small-business operators, and
founders whose local folders are the system of record. The common thread: *"I need this done
safely, and I need proof of what changed."*

Ghost is **not** a generic AI agent or an RPA clone. AI may suggest categories, filenames, or
steps; deterministic code executes only the plan you approved — on your machine — and writes
undo data before it mutates anything.

---

## How it works

```text
Select folder → Scan → Propose plan → Review → Approve → Move/Rename → Audit → Undo
```

- **Select folder** — the user chooses the source folder; no background file watching.
- **Scan** — Ghost reads local file metadata inside the approved boundary.
- **Propose plan** — deterministic planner suggests folders, moves, and dated renames.
- **Review** — every before/after path is visible, with conflicts and low-confidence items called out.
- **Approve** — nothing mutates until the final plan is explicitly approved.
- **Move/Rename** — executor re-checks policy and boundaries before applying changes.
- **Audit** — the run is saved into a local audit chain.
- **Undo** — reversible operations write undo data before execution.

See [docs/product-direction.md](docs/product-direction.md) for the Organizer-first product decision,
and [docs/automation-strategy.md](docs/automation-strategy.md) for how the same pipeline expands
into auditable workflows beyond filing.

## What Ghost does today

### Organizer (flagship)

Point Ghost at a folder or Zone. It scans read-only, proposes moves and dated renames, detects
conflicts, and **never overwrites or deletes silently**. Approve → execute → audit chain →
one-click undo. This is the product wedge to improve first.

### Record → approve → replay → verify (trust-core capability)

Record/replay remains part of Ghost's long-term trust core, but it should not be the default
product promise until reliability, verification, receipts, and undo paths are stronger across real
macOS and Windows workflows.

### Trust pipeline

Every meaningful mutation passes through:

```text
Intent → Plan → Policy check → User approval → Execution → Audit log → Undo path
```

- **Ghost Guard** — local, deterministic safety: suppresses capture of password/OTP/payment fields
  and audits risk before replay. No network, no model.
- No silent delete, overwrite, upload, or send.
- AI proposes only; approved plans execute in deterministic code.

## Where this is going

The trust pipeline above is horizontal; the roadmap turns it into auditable
**playbooks** for specific teams. These are direction, not shipped features yet:

- **Vertical playbooks** — e.g. accounting/bookkeeping month-end close: scan, rename by
  period, file per client, reconcile against an expected-docs checklist, and produce a
  signable close report. See [docs/vertical-accounting-close.md](docs/vertical-accounting-close.md).
- **Scheduled runs** — recurring "propose a plan every month-end," still resolving to an
  approval and a sealed receipt. See [docs/scheduled-runs-and-team-audit.md](docs/scheduled-runs-and-team-audit.md).
- **Team audit layer** — aggregate sealed receipts (never files) for a reviewer or auditor.

Strategy and guardrails: [docs/automation-strategy.md](docs/automation-strategy.md).

## Connect an AI assistant (MCP)

Claude Desktop and Cursor can talk to Ghost over **local stdio** (`ghost mcp serve`). The assistant
lists and previews routines; you approve in Ghost; only then can it execute and read a receipt:

```text
list → preview → request approval → (approve in Ghost) → execute → receipt
```

ChatGPT and marketplace listings are **not** supported yet. Pairing + walkthrough:
[docs/claude-ghost-demo.md](docs/claude-ghost-demo.md) ·
[docs/mcp-integration.md](docs/mcp-integration.md).

## Privacy by default

No camera. No microphone. No hidden screen capture. No background email, browser, or tab reading.
No raw secret capture. No cloud-first storage — workflow and organizer data stay local and encrypted
at rest. Keyboard and pointer are captured **only** while you explicitly record or an approved
replay runs.

## Pricing

Priced per seat for teams; the number isn't being advertised while the product is in
developer preview.

## Download

Published release: **[v2.0.3](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)**
(macOS notarized; Windows unsigned). Source may be ahead of that tag — advertise the GitHub
Release with assets, not the version in `Cargo.toml`.

- **macOS** (Apple Silicon & Intel, macOS 12+) — [Ghost.dmg](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)
- **Windows 10/11** — [Ghost_Setup.exe](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)

Verify against `SHA256SUMS.txt` and cosign signatures —
[docs/VERIFY_DOWNLOADS.md](docs/VERIFY_DOWNLOADS.md).

**macOS Gatekeeper:** if the app is blocked on first launch, right-click → **Open**, or:

```bash
xattr -dr com.apple.quarantine /Applications/Ghost.app
```

## Build from source

Rust + Tauri core; Vite-bundled frontend. Prerequisites: Rust (stable), Node.js 20+
(Linux GTK/WebKit deps in `AGENTS.md`; macOS needs Xcode CLT).

```bash
npm install
cargo install tauri-cli --version "^2.0" --locked

make ci                              # fmt-check + clippy + test
make build                           # cargo tauri build --no-bundle
make dev                             # cargo tauri dev (pass -- --bin ghost if needed)
cargo tauri build                    # full installer
```

See `AGENTS.md` / `CLAUDE.md` for the full validation matrix and product contract.

> `apps/macos/` is an early native SwiftUI scaffold over the same Rust trust core. Not the
> shipping surface yet.

## What Ghost is not

- Not a generic autonomous AI agent or “context layer” for your desktop.
- Not an RPA clone or blind macro recorder — you inspect, approve, and verify every run.
- Not cloud-first — your files and workflow data stay on your machine by default.
- Not a silent computer takeover — capture and execution are explicit, interruptible, and reversible.
- Not a multi-provider LLM routing platform — which model runs underneath isn't the point.

Ghost may propose anything; it only does what you approve, inside a boundary you control,
and it can undo the run.

## License

MIT License — see [LICENSE](LICENSE).
