# Ghost

[![Download](https://img.shields.io/badge/Download-Latest-14b8a6?style=flat-square)](https://github.com/mohabbis/ghost/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![macOS](https://img.shields.io/badge/macOS-12+-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10/11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

> **Automate the busywork. Catch the errors.** Local-first desktop automation for financial-services teams.

Finance and operations teams still move numbers between spreadsheets and systems by hand —
and one mistyped or mis-pasted figure is expensive to find later. Ghost records that repetitive
data entry once, then **replays it on your Mac or PC and verifies every value against the one you
approved**. A mismatch stops the run and surfaces the exception. Every change is audited and reversible.

Ghost is **not** an autonomous cloud agent and **not** a blind macro recorder. AI may suggest;
deterministic code executes only the plan you approved, on your machine, with your files never
leaving it — and it checks its own work before it asks you to sign off.

---

## The wedge

The error that hides between two spreadsheets: a figure copied into the wrong row, a transposed
digit, a stale value no one re-checked. Ghost automates the transfer **and** checks the result,
so the exception surfaces at the keystroke instead of in the close:

```
Record → Review → Approve → Replay → Verify → Recover
```

- **Record** — capture the exact re-keying or copy-across you do every period.
- **Review** — raw input is compressed into *readable steps* ("set B7 · Revenue → 48,210.00"),
  not opaque mouse coordinates. Typed text is redacted by default; secrets are dropped.
- **Approve** — deny-by-default. Every step, and the value it will write, needs your explicit consent.
- **Replay** — deterministic code runs the approved plan, preferring semantic UI resolution
  (macOS Accessibility `set_value`) over raw coordinates.
- **Verify** — each step confirms the approved value actually landed in the field; observed ≠
  expected halts the run, and the whole run is sealed into an execution receipt.
- **Recover** — undo data is written before any change, so one click puts things back.

## What Ghost does today

### 🎬 Record → replay → verify (the flagship)
Record a data-entry routine once. Ghost compresses the raw input into semantic steps you can
read and approve, then replays them — preferring **semantic UI resolution** (macOS Accessibility
`set_value`, window-relative targeting, pixel template match) with raw coordinates only as a
last-resort fallback. Each step is verified as it runs (the value you approved actually landed),
every replay traces how its target resolved, the run is interruptible, and undo data is written first.

> **Honest scope:** verification today is *per-step* — Ghost confirms the value it wrote is the value
> you approved. Full **source-vs-destination reconciliation** (comparing each transferred figure
> against its source of truth and flagging every discrepancy) is the next milestone, not shipped yet.
> See the roadmap below.

### 🗂 Organizer — keep the outputs tidy
Point Ghost at a folder. It scans read-only, proposes moves and dated renames by type and
reporting period, detects conflicts, and **never overwrites or deletes silently**. Approve the
plan and it executes with a tamper-evident audit chain and one-click undo.

### 🔒 Local safety, no black box
- **Deterministic event compression** — turns raw clicks/keystrokes into a reviewable timeline.
- **Ghost Guard** — a local, deterministic safety layer that suppresses capture of password/OTP/
  payment fields and audits a plan for risk *before* replay. No network, no model.
- **Tamper-evident audit + undo** — every mutating run seals a hash-chained audit log and writes an
  undo journal first, all on-device.

Additional subsystems exist in the codebase (on-device OCR/ID parsing, a local semantic-memory
graph, an MCP approval surface, experimental suggestion-only AI providers, and optional
Microsoft/Google identity sign-in). They are **off the default surface** — experimental pieces
sit behind a Cargo feature flag and are not part of the shipping financial-services workflow above.

## Privacy by default

No camera. No microphone. No hidden screen capture. No background email, browser, or tab reading.
No raw secret capture. No cloud-first storage — workflow and organizer data stay local and encrypted
at rest. Keyboard and pointer are captured **only** while you explicitly record or an approved replay runs.

## Download

Current release: **v2.0.3** (macOS notarized; Windows unsigned). Get the latest build for your platform:

- **macOS** (Apple Silicon & Intel, macOS 12+) — [download the `.dmg`](https://github.com/mohabbis/ghost/releases/latest)
- **Windows 10/11** — [download the installer](https://github.com/mohabbis/ghost/releases/latest)

Verify your download against `SHA256SUMS.txt` and the cosign signatures attached to each
[release](https://github.com/mohabbis/ghost/releases/latest) — step-by-step commands in
[docs/VERIFY_DOWNLOADS.md](docs/VERIFY_DOWNLOADS.md).

**macOS Gatekeeper note:** if macOS blocks the app on first launch, right-click the app and choose
**Open**, or clear the quarantine attribute:

```bash
xattr -dr com.apple.quarantine /Applications/Ghost.app
```

## Build from source

Ghost's shipping app is built with **Rust + Tauri** (Rust core + web frontend bundled by Vite).

```bash
# Prerequisites: Rust (stable), Node.js 20+.
# Linux also needs GTK/WebKit dev libs (see AGENTS.md); macOS needs Xcode CLT.
npm install
cargo install tauri-cli --version "^2.0" --locked
cargo tauri build          # full installer
cargo tauri build --no-bundle   # compile without packaging
```

Common checks (see `AGENTS.md` / `CLAUDE.md` for the full list):

```bash
cargo test  --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

> `apps/macos/` contains an early **native SwiftUI** shell (a Ghost 2.0 preview scaffold) that
> talks to the same Rust trust core over a JSON bridge. It is not the shipping surface yet.

## What Ghost is not

- ❌ Not an autonomous cloud agent that reads your inbox and acts on its own.
- ❌ Not a black box — every action names the rule that fired and is auditable.
- ❌ Not cloud-first — your workflows and files stay on your machine.
- ❌ Not a silent macro recorder — you review and approve before anything replays.

Ghost may propose anything; it only does what you approve, inside a boundary you control.

## License

MIT License — see [LICENSE](LICENSE).
