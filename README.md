# Ghost

[![Download](https://img.shields.io/badge/Download-v2.0.3-14b8a6?style=flat-square)](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![macOS](https://img.shields.io/badge/macOS-12+-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)
[![Windows](https://img.shields.io/badge/Windows-10/11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

> **Ghost safely organizes messy local folders without the data ever leaving the machine.**
> It scans a folder you choose, proposes an exact move/rename plan, waits for approval,
> executes deterministically, writes an audit log, and can undo the run.

Ghost starts with a job almost every operator understands: making sense of messy Downloads,
client folders, month-end exports, PDFs, screenshots, and handoff packets without risking a
silent delete or overwrite. The product wedge is **Ghost Organizer** because it proves the
trust pipeline in minutes before asking users to trust broader desktop replay.

**Who it's for first:** people accountable for recurring operational files — small-business
operators, finance/accounting/admin staff, consultants handling client deliverables, and founders
whose local folders have become the system of record.

Ghost is **not** a generic AI agent, a context layer, or an RPA clone. AI may suggest categories
or filenames; deterministic code executes only the plan you approved — on your machine — and
writes undo data before mutation.

---

## The wedge

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

See [docs/product-direction.md](docs/product-direction.md) for the Organizer-first product decision.

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

$79/month per seat. Flat — no tiers, no "contact sales."

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
- Not an RPA clone or blind macro recorder — you inspect, approve, and verify.
- Not cloud-first — workflows and files stay on your machine by default.
- Not a silent computer takeover — capture and replay are explicit and interruptible.
- Not "workflow automation" or an "operating system" — one product, one workflow.
- Not a multi-provider LLM routing platform — which model runs underneath isn't the point.

Ghost may propose anything; it only does what you approve, inside a boundary you control.

## License

MIT License — see [LICENSE](LICENSE).
