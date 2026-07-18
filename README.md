# Ghost

[![Download](https://img.shields.io/badge/Download-v2.0.3-14b8a6?style=flat-square)](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![macOS](https://img.shields.io/badge/macOS-12+-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)
[![Windows](https://img.shields.io/badge/Windows-10/11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

> **Automate the busywork. Catch the errors.** Local-first desktop automation for financial-services teams.

Finance and ops still move numbers between spreadsheets and systems by hand. One transposed digit
can hide until the close. Ghost records that transfer once, then **replays it on your Mac or PC and
verifies every value against the one you approved**. A mismatch halts the run. Every change is
audited, receipted, and reversible.

Ghost is **not** a generic AI agent, a context layer, or an RPA clone. AI may propose; deterministic
code executes only the plan you approved — on your machine — and checks its own work before you
sign off.

---

## The wedge

```text
Record → Inspect → Approve → Replay → Verify → Receipt → Undo
```

- **Record** — capture the re-keying or copy-across you do every period (visible capture only).
- **Inspect** — raw input compresses into readable steps ("set B7 · Revenue → 48,210.00"), not
  opaque coordinates. Typed text is redacted by default; secrets are dropped.
- **Approve** — deny-by-default. Every step, and the value it will write, needs your consent.
- **Replay** — deterministic code runs the approved plan (semantic UI resolution preferred;
  coordinates are fallback only).
- **Verify** — *per-step*: confirm the approved value actually landed; observed ≠ expected halts
  the run.
- **Receipt** — the whole run seals into an on-device execution receipt.
- **Undo** — undo data is written before reversible changes, so one click puts things back.

**Honest scope:** verification today is per-step (the value you approved landed in the field).
Full **source-vs-destination reconciliation** is roadmap — not shipped.

## What Ghost does today

### Record → approve → replay → verify (flagship)

Record a data-entry routine once. Ghost compresses it into semantic steps you review and approve,
then replays them — preferring semantic UI resolution (macOS Accessibility `set_value`,
window-relative targeting, pixel template match) with coordinates as last resort. Each step is
verified as it runs, the run is interruptible, a receipt is sealed, and undo data is written first.

### Organizer (supporting)

Point Ghost at a folder. It scans read-only, proposes moves and dated renames, detects conflicts,
and **never overwrites or deletes silently**. Approve → execute → audit chain → one-click undo.
Supporting capability for tidy outputs — not the headline product.

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

Ghost may propose anything; it only does what you approve, inside a boundary you control.

## License

MIT License — see [LICENSE](LICENSE).
