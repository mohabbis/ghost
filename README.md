# Ghost

[![Download](https://img.shields.io/badge/Download-Latest-8d7bff?style=flat-square)](https://github.com/mohabbis/ghost/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![Release](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/release.yml?style=flat-square&label=Release)](https://github.com/mohabbis/ghost/actions/workflows/release.yml)
[![macOS](https://img.shields.io/badge/macOS-12%2B-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10%2F11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

> **Ghost turns repeated computer work into safe, reusable, permission-bounded routines.**

Record a task once. Ghost shows you exactly what it saw, checks it for risk, asks for your approval, and replays it — with live progress, an audit trail, and undo.

Ghost is **local-first desktop automation** for macOS and Windows. Not a chatbot, not a cloud service, not an autonomous agent. It's a trusted layer between you and your computer: it watches only when you tell it to, executes only what you've approved, and keeps everything on your machine.

## The problem

Every week you do the same twenty minutes of clicking: filing downloads, renaming reports, moving data between a portal and a spreadsheet, filling the same form. It's not hard — it's just *yours*, every week, forever. The tasks are too fiddly for scripts, too specific for off-the-shelf tools, and too sensitive to hand to something you can't inspect.

Ghost exists for exactly that work. It makes one painful, repetitive desktop workflow reliable enough that you trust it weekly — and it earns that trust by showing its work at every step.

## How it works

```text
Record → Inspect → Approve → Replay → Audit → Undo
```

1. **Record** — capture clicks, keystrokes, scrolls, and timing while you do the task once. Passwords and secure fields are suppressed at capture time.
2. **Inspect** — the recording becomes a readable step timeline, with fragile steps flagged before you ever run them.
3. **Approve** — Ghost Guard audits the workflow for sensitive apps, destructive actions, and credential inputs. High-risk workflows need explicit confirmation or are blocked.
4. **Replay** — run it with live per-step status, pause/resume, speed control, and an emergency stop that always works.
5. **Audit** — every run leaves a durable local record: what ran, how long it took, what failed and why.
6. **Undo** — reversible operations write their undo data *before* they execute.

## Get Ghost

| Platform | Download |
|----------|----------|
| macOS (Apple Silicon + Intel) | [Ghost.dmg](https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg) |
| Windows 10 / 11 (64-bit) | [Ghost_Setup.exe](https://github.com/mohabbis/ghost/releases/latest/download/Ghost_Setup.exe) |

> **Note:** current builds are developer-preview quality. macOS builds may be ad-hoc signed; if macOS blocks the app, approve it under **System Settings → Privacy & Security**. Notarized releases are in progress.

## Your first five minutes: clean up Downloads

The fastest way to understand Ghost is to watch it handle a real, boring task:

1. **Record** — open Downloads, sort files, rename a few, move them into folders.
2. **Review** — Ghost replays the recording back to you as human-readable steps.
3. **Approve** — Ghost Guard confirms there are no deletions, no overwrites, no surprises.
4. **Replay** — run it on the next messy folder and watch each step execute.
5. **Audit** — open the run log: every move and rename, timestamped, undoable.

The same flow powers **Ghost Organizer**, the built-in file-cleanup workflow: pick a folder, preview every proposed move, approve the plan, and Ghost executes it inside boundaries you set — writing an audit log and an undo journal before it touches anything. Ghost never silently deletes and never silently overwrites.

## What Ghost does

### A review timeline, built after every recording
Raw input events are deterministically compressed into semantic steps — "Clicked *Save* in Notes", "Typed text (redacted)", "Waited 2s" — so you review intent, not noise. Coordinate-only and low-confidence steps are flagged for your attention.

### A preview of every step, before anything runs
Dry-run any workflow and Ghost lists exactly what each step will do — which element it will click, where, and which steps would fall back to raw coordinates (the ones most likely to break if a window moves). Nothing executes during a preview.

### A safety check that runs before every replay
Ghost Guard scans each workflow for sensitive apps (password managers, banking, terminals, system settings), destructive actions (delete, send, submit, pay, install), and credential-shaped input. Risky workflows require explicit confirmation; workflows that captured secret-like input are blocked from replay outright.

### Replay you can watch — and interrupt
Live step counter, pause/resume, playback speed control, and cancel-anytime. Replay one step at a time when you're building trust in a new workflow, and when a run fails, retry from the failed step instead of starting over.

### Boundaries you draw, permissions you grant
Zones define where Ghost may operate (e.g. Downloads → Documents/School). Capabilities control what it may do there — read, create, move, rename. Delete is blocked by default. Everything else is denied unless you've allowed it.

### A durable record of every run
Replay history shows every run's status, duration, and failure reason. Organizer runs additionally persist a full audit log and undo journal, so you can always answer "what did Ghost change?" — and reverse it.

### Local by default, private by design
Workflows, logs, and settings live on your machine, optionally encrypted at rest (Argon2 + AES-GCM) behind a local password. No account. No cloud dependency. No telemetry unless you explicitly opt in. No camera, no microphone, no hidden screen capture — input capture happens only during explicit recording or approved replay.

## Who Ghost is for

- **Bookkeepers and practice admins** who download client invoices, receipts, and statements every week and need local filing that is previewed, audited, and reversible.
- **Students and knowledge workers** drowning in downloads, attachments, and misnamed files.
- **Builders and tinkerers** who want a desktop automation layer they can actually read, audit, and extend.

If you want an autonomous agent that acts on your behalf while you're away — Ghost is deliberately not that. Every meaningful action passes through your approval.

## The safety model

One principle, enforced in code:

> **Ghost may suggest anything, but it only does what you have approved, inside boundaries you control.**

Every meaningful operation passes through the same pipeline — no exceptions for file operations, replay, or anything else:

```text
Intent → Plan → Policy check → Approval → Execution → Audit log → Undo path
```

- **Deny by default.** The policy engine refuses anything not explicitly allowed by your Zones and capabilities.
- **Deterministic execution.** Suggestions (including any AI-assisted ones) never execute directly; only reviewed, approved plans do.
- **Suppressed secrets.** Secure-field input is never retained — not in recordings, not in previews, not in logs.
- **Experimental means off.** AI, cloud, and observer features are compiled out of default builds entirely (`experimental` Cargo feature), not just hidden in the UI.

## Under the hood

- **Shell**: Tauri 2 — Rust backend, vanilla HTML/CSS/JS frontend
- **Capture**: macOS CGEventTap + Accessibility API; Windows input hooks
- **Replay**: semantic element re-resolution first, recorded coordinates as fallback; interruptible at every step
- **Storage**: local JSON + SQLite (Zones, rules, run history) — no external services
- **Encryption**: Argon2 key derivation + AES-GCM for at-rest workflow protection

| Module | Purpose | Status |
|--------|---------|--------|
| `core/compression/` | Raw events → reviewable semantic steps | Stable |
| `core/guard.rs` | Ghost Guard pre-replay risk audit | Stable |
| `core/dry_run.rs` | Step-by-step replay preview | Stable |
| `policy/` | Deny-by-default capability engine | Stable |
| `organizer/` | Safe file-cleanup workflow | Stable |
| `audit/` | Audit logs + undo journals | Stable |
| `platform/` | macOS / Windows capture & replay backends | Stable |
| `core/ai.rs`, `core/cloud.rs` | Suggestions, sync | Experimental, compiled out by default |

See [`IMPLEMENTATION.md`](IMPLEMENTATION.md) for the full, honest feature-status matrix.

## Where Ghost is today

**Early-stage but functional.** Working now:

- ✅ Record and replay workflows on macOS and Windows
- ✅ Ghost Guard safety audits before every replay
- ✅ Dry-run preview, live per-step replay status, step-by-step replay, retry-from-failed-step
- ✅ Permission-bounded file organization (Organizer) with audit and undo
- ✅ Local, optionally encrypted workflow storage
- ✅ Replay history with failure reasons

**Not yet production-ready:**

- ⚠️ Cross-app replay reliability — semantic targeting still falls back to coordinates in some apps
- ⚠️ Richer locator strategies (window titles, window-relative positions) — in progress
- ⚠️ Release signing — macOS notarization in progress

This is a **technical preview** for builders, students, and operators who want to experiment with trustworthy local automation. It is not yet something to run unattended over work you can't afford to redo.

## Roadmap

1. **Target resilience** *(now)* — multiple locator strategies per click, semantic lookup before coordinates, fallback decisions explained in the UI, a success/failure trace for every replay.
2. **Benchmark reliability** — a canonical-workflow suite with replay success rate as the north-star metric.
3. **Distribution quality** — signed/notarized builds, clean onboarding, honest docs.
4. **Constrained intelligence** *(last, gated)* — suggestion-only naming, summaries, and failure explanations. AI proposes; deterministic code executes.

## FAQ

**Does my data leave my machine?**
No. Workflows, logs, and settings are local. There's no account and no server. Telemetry exists but is off unless you opt in.

**Can Ghost delete my files?**
Not silently, and in Organizer not at all — delete is blocked by default, conflicts are detected, and every mutation is previewed, approved, audited, and undoable.

**What about my passwords?**
Secure-field input is suppressed at capture time and never retained. Ghost Guard additionally blocks replay of workflows that appear to contain stored secrets.

**Can it run while I'm away?**
Replay runs when you start it and stays interruptible the whole time; risky workflows require your confirmation before they run at all. Unattended, scheduled execution is deliberately not a current feature.

**Why not just use a macro recorder?**
Raw macros replay blind coordinates and break the moment a window moves — and they'll happily type your password into the wrong field. Ghost reviews, guards, re-resolves targets semantically, and leaves an audit trail.

## Development setup

```bash
# Install Rust, then the Tauri CLI
cargo install tauri-cli --version "^2.0" --locked

# Run the app
cd src-tauri && cargo tauri dev

# Validate
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

# Build installers
cargo tauri build
```

`make ci`, `make check`, `make build`, and `make dev` wrap the common flows.

### Permissions

- **macOS**: Accessibility (observe/replay) and Input Monitoring (keyboard capture) under **System Settings → Privacy & Security**; restart the app after granting.
- **Windows**: native input hooks; apps running elevated (as administrator) may not be controllable from a user-level Ghost process.

## Known limitations

- Some workflows still depend on screen coordinates; moving windows can break them (this is the current focus — see Roadmap).
- Works best with standard native apps; Electron apps, games, and VMs have limited support.
- Fast-changing or network-dependent UIs may need manual wait conditions.
- No mobile support (OS restrictions make it unlikely, ever).
- UI element detection works best in English-language apps.

## Contributing

Contributions welcome, focused on: replay reliability, semantic target resolution, debugging tools, safety/policy hardening, docs and examples.

Not accepting: feature bloat, cloud-dependent features (local-first is non-negotiable), or autonomous execution modes (user approval is the product).

Read [`AGENTS.md`](AGENTS.md) first — it's the binding contract for humans and AI agents alike. Architecture details live in [`docs/`](docs/).

## License

MIT — see [LICENSE](LICENSE).

---

**Boringly trustworthy first, then powerful.** Ghost earns the right to automate your work by showing you everything it does — before, during, and after.
