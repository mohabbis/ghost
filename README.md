# Ghost

[![Download](https://img.shields.io/badge/Download-Latest-14b8a6?style=flat-square)](https://github.com/mohabbis/ghost/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![macOS](https://img.shields.io/badge/macOS-12+-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10/11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

> **Approve before it acts.** Local-first desktop automation for macOS and Windows.

Ghost turns repeated computer work — cleaning up folders, filing documents, replaying
multi-step tasks — into **safe, reviewable, permission-bounded routines**. It can be smart
about *proposing* what to do, but nothing touches your files or apps until you approve it,
and every change is audited and reversible.

Ghost is **not** an autonomous cloud agent. AI may suggest; deterministic code executes
only the plan you approved, on your machine, with your files never leaving it.

---

## One pipeline, every task

Ghost 2.0 consolidates everything it does onto a single **Action Plan runtime**. Whether
you point it at a messy folder, record a task, or accept a proposal from an AI assistant,
the work compiles into the same reviewable plan and runs through the same trust pipeline:

```
Capture → Review → Approve → Execute → Verify → Recover
```

- **Capture** — scan a folder, record a task, or receive a proposal.
- **Review** — raw input is compressed into *readable semantic steps* ("move invoice → Finance",
  "focus TextEdit, set value"), not opaque mouse coordinates. Typed text is redacted; secrets are dropped.
- **Approve** — deny-by-default. Risky actions need explicit consent, at the trust level you set per folder.
- **Execute** — deterministic code runs the approved plan. No silent delete, no silent overwrite.
- **Verify** — every step records *expected vs. observed*, sealed into a per-run execution receipt.
- **Recover** — undo data is written before any reversible change, so one click puts things back.

That one pipeline is what keeps Ghost powerful without becoming a black box.

## What Ghost does today

### 🗂 Ghost Organizer — the wedge
Point Ghost at `~/Downloads` (or any folder). It scans read-only, proposes moves and dated
renames by file type and reporting period, detects conflicts, and **never overwrites or
deletes silently**. Approve the plan and it executes with a tamper-evident audit chain and
one-click undo. Filing profiles adapt to the work: software artifacts (test reports, coverage,
build logs, traces), finance & operations, and student coursework.

### 🎬 Ghost Routines — record, review, replay
Record a multi-step task once. Ghost compresses the raw input into semantic steps you can
read and approve, then replays them — preferring **semantic UI resolution** (macOS Accessibility
`set_value`, window-relative targeting, pixel template match) with raw coordinates only as a
last-resort fallback. Every replay traces how each target resolved, is interruptible, and writes
undo data first.

### 🧠 Intelligent, but on your terms
- **Deterministic event compression** — turns raw clicks/keystrokes into a reviewable timeline.
- **Ghost Guard** — a local, deterministic safety layer that suppresses capture of password/OTP/
  payment fields and audits a plan for risk *before* replay. No network, no model.
- **Ghost Atlas** — a local, offline semantic-memory graph for recall across routines. Retrieval
  is deterministic and lexical (character/word hashing) — it runs entirely on-device with no model
  download and no network. "Forgetting" is a reversible archive flag, never a delete.
- **On-device OCR & ID parsing** — local OCR (macOS Vision / Windows OCR) over images you supply,
  plus deterministic ID-document field parsing. No image ever leaves the machine.
- **Suggestion-only AI planning** *(experimental, opt-in, off by default)* — optional OpenAI/Anthropic
  providers that *propose* plans. They never execute; deterministic code still runs only what you approve.
- **MCP surface** — an external AI assistant can *propose* an action, but a locally-issued, single-use,
  plan-bound approval token is required before anything runs through the normal trust pipeline.

### 🔌 Optional, disclosed integrations
Account sign-in (Microsoft / Google) is **identity-only** and opt-in — signing in does not move
your data anywhere. Stack integrations (e.g. Power BI audit export) are separately consented,
scoped grants and are experimental. Ghost ships with no client IDs of its own; you configure your own.

## Privacy by default

No camera. No microphone. No hidden screen capture. No background email, browser, or tab reading.
No raw secret capture. No cloud-first storage — workflow and organizer data stay local and encrypted
at rest. Keyboard and pointer are captured **only** while you explicitly record or an approved replay runs.

## Download

Get the latest release for your platform:

- **macOS** (Apple Silicon & Intel, macOS 12+) — [download the `.dmg`](https://github.com/mohabbis/ghost/releases/latest)
- **Windows 10/11** — [download the installer](https://github.com/mohabbis/ghost/releases/latest)

Verify your download against `SHA256SUMS.txt` attached to each [release](https://github.com/mohabbis/ghost/releases/latest).

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
