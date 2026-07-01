# Ghost

[![Download](https://img.shields.io/badge/Download-Latest-8d7bff?style=flat-square)](https://github.com/mohabbis/ghost/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/rust.yml?style=flat-square&label=Build)](https://github.com/mohabbis/ghost/actions/workflows/rust.yml)
[![Release](https://img.shields.io/github/actions/workflow/status/mohabbis/ghost/release.yml?style=flat-square&label=Release)](https://github.com/mohabbis/ghost/actions/workflows/release.yml)
[![macOS](https://img.shields.io/badge/macOS-12%2B-black?style=flat-square&logo=apple)](https://github.com/mohabbis/ghost/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10%2F11-0078d4?style=flat-square&logo=windows)](https://github.com/mohabbis/ghost/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

> **Ghost turns repetitive desktop work into safe, reviewable automations.**

Record a task once. Ghost explains the workflow, checks the risks, asks for approval, and replays it across your apps with permissions, debugging, and audit logs.

Ghost is **local-first desktop automation**—not an AI assistant, not a chatbot, not a cloud service. It's a trusted layer that watches user-approved work, converts it into reusable workflows, and executes them safely with explicit permission controls.

## Why Ghost Exists

Tools like Bond organize your work in the cloud. Ghost **operates your computer safely** at the desktop execution layer.

| Bond | Ghost |
|------|-------|
| Cloud-based AI chief of staff | Local-first desktop automation |
| Summarizes email, Slack, calendar | Records and replays actual computer actions |
| Suggests what to do | Actually does it (with approval) |
| Lives in your browser | Runs natively on macOS and Windows |
| Generic productivity assistant | Focused on repetitive desktop chores |

Ghost wins by owning the **desktop execution layer**: browser forms, Finder/File Explorer, PDFs, spreadsheets, downloads, portals, and recurring admin tasks—with full permission control, audit logs, and rollback.

## Quick Start

| Platform | Download |
|----------|----------|
| macOS Apple Silicon + Intel | [Ghost.dmg](https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg) |
| Windows 10 / 11 64-bit | [Ghost_Setup.exe](https://github.com/mohabbis/ghost/releases/latest/download/Ghost_Setup.exe) |

> **Note:** Current builds are developer-preview quality. macOS builds may be ad-hoc signed; if macOS blocks the app, open **System Settings → Privacy & Security** and approve it. Fully notarized releases are in progress.

## First Workflow: Downloads Folder Cleanup

The best way to understand Ghost is to see it handle a real, boring task:

1. **Record**: Open Downloads, sort files by type, rename selected files, move into folders
2. **Review**: Ghost explains the workflow as human-readable steps
3. **Approve**: Ghost Guard audits for risks (no deletions, no overwrites)
4. **Replay**: Run the workflow on another messy folder with visible progress
5. **Audit**: View the run log showing every file moved, renamed, and approved

This demonstrates:
- ✅ Native desktop execution (Finder/File Explorer)
- ✅ Local-first workflow storage
- ✅ Ghost Guard safety audit
- ✅ Permission boundaries (Zone-based)
- ✅ Replay with visible progress
- ✅ Audit trail with undo support

## Core Features

### Record Once
Capture clicks, keystrokes, file paths, window titles, and timing while you work. Ghost suppresses sensitive inputs (passwords, payment fields) automatically.

### Review Workflow
Every recording becomes an editable step list. Rename steps, disable fragile ones, add wait conditions, and inspect targets before replay.

### Ghost Guard Safety Layer
Before any replay, Ghost audits the workflow for:
- Sensitive apps (password managers, banking, terminals)
- Destructive actions (delete, overwrite, send, submit)
- Credential inputs (passwords, API keys, OTPs)
- Low-confidence targets (coordinate-only clicks)

High-risk workflows require explicit confirmation or are blocked entirely.

### App-Level Permissions
Control exactly which apps and folders Ghost can touch:
- **Zones**: Define approved boundaries (e.g., Downloads → Documents/School)
- **Capabilities**: Grant read, create, move, rename (delete blocked by default)
- **Per-workflow overrides**: Tighten permissions for specific automations

### Replay Console
Run workflows with:
- Start/pause/stop controls
- Speed adjustment (0.5x–2.0x)
- Step-by-step status
- Approval checkpoints for risky actions
- Emergency cancel at any time

### Audit Vault
Every run produces a durable log:
- Timestamp, duration, apps touched
- Files changed, text entered (redacted where sensitive)
- Approvals requested, steps passed/failed/skipped
- Error explanations and rollback options

### Workflow Debugger
When automation fails, understand why:
- Failed step highlighting
- Target comparison (expected vs actual)
- Retry from failed step
- Skip problematic steps
- Fragile step warnings (coordinate-only, timing-dependent)

## Architecture

```text
Raw Input Capture → Deterministic Timeline → Semantic Workflow → Ghost Guard Audit
       ↓
Permission Check → User Review → Native Execution → Run Log → Failure Recovery
```

Each stage is independently testable and inspectable.

### Key Modules

| Module | Purpose | Status |
|--------|---------|--------|
| `core/events.rs` | Input event schema | Stable |
| `core/guard.rs` | Ghost Guard risk audit | Stable |
| `core/compression/` | Raw events → semantic steps | Stable |
| `policy/` | Capability-based permissions | Stable |
| `storage/` | SQLite Zones, folder rules, execution history | Stable |
| `organizer/` | File organization workflow | Stable |
| `audit/` | Audit logs and undo journals | Stable |
| `platform/macos.rs` | macOS accessibility/event APIs | Stable |
| `platform/windows.rs` | Windows input hooks/UI automation | Stable |
| `core/ai.rs` | AI workflow suggestions | Experimental |
| `core/cloud.rs` | Cloud sync | Experimental |

### Technology Stack

- **Frontend**: Vanilla HTML, CSS, JavaScript (Tauri 2 webview)
- **Backend**: Rust (Tauri 2 commands)
- **Desktop Shell**: Tauri 2
- **Storage**: SQLite (bundled, no external dependencies)
- **Encryption**: Argon2 + AES-GCM for local auth
- **Permissions**: macOS Accessibility + Input Monitoring, Windows UI Automation

## Safety Model

Ghost follows one principle:

> **Ghost may suggest anything, but it only does what the user has approved inside boundaries the user controls.**

### Canonical Trust Pipeline

Every meaningful operation passes through:

```text
Intent → Plan → Policy → Approval → Execution → Audit → Undo
```

No shortcuts exist for file operations, workflow replay, browser actions, or network actions.

### Blocked by Default

- Password managers (1Password, Bitwarden, Keychain)
- Banking/financial portals
- Payment pages
- Terminal/shell commands
- System settings
- Private messaging apps
- Healthcare/medical records
- Legal/school records

### Require Confirmation

- Sending emails/messages
- Deleting files (blocked entirely in Organizer MVP)
- Overwriting files
- Moving large folders
- Editing system settings
- Form submissions
- App installs/uninstalls

### Allowed with Permission

- File organization inside approved Zones
- Attachment downloads
- Browser navigation (non-sensitive sites)
- Spreadsheet/data entry
- PDF handling
- Renaming/moving files

## Current Status

**Ghost is early-stage but functional.** The current build focuses on:

- ✅ Recording and replaying simple workflows
- ✅ Ghost Guard safety audits
- ✅ Permission-bounded file organization (Organizer)
- ✅ Local workflow storage with encryption
- ✅ Audit logs and undo support
- ✅ macOS and Windows platform support

**Not yet production-ready:**
- ⚠️ Cross-app replay reliability (works best within single apps)
- ⚠️ Semantic target resolution (still depends on coordinates for some apps)
- ⚠️ Workflow debugger UI (basic timeline exists, advanced debugging coming)
- ⚠️ Release signing (macOS notarization in progress)

Honest positioning: This is a **technical preview** useful for builders, students, and operators who want to experiment with local desktop automation. It is not yet enterprise-ready.

## Roadmap

### Phase 1: Stabilize Foundation (Now)
- Reliable recording/replay across common apps
- Clear command risk inventory
- Workflow inspector with step editing
- Run log viewer with failure explanations

### Phase 2: Add Trust & Safety (Now)
- Ghost Guard UI improvements
- Per-app permission controls
- Approval checkpoint system
- Redaction review for suppressed inputs

### Phase 3: Debuggability (Next)
- Step-by-step workflow debugger
- Failed step explanations
- Retry/skip/reorder steps
- Screenshot comparison for failed targets

### Phase 4: Semantic Intelligence (Later)
- Accessibility-based target resolution
- Window/control metadata lookup
- Dynamic value handling
- Workflow templates for common tasks

### Phase 5: Polish for Credibility (Later)
- Signed/notarized macOS builds
- Windows code signing
- Demo video/GIFs
- Examples gallery
- Test coverage reports

## Installation

### Development Setup

1. Install Rust toolchain:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Install Tauri CLI:
   ```bash
   cargo install tauri-cli --version "^2.0" --locked
   ```

3. Run the desktop app:
   ```bash
   cd src-tauri
   cargo tauri dev
   ```

4. Build distributable installers:
   ```bash
   cargo tauri build
   ```

### Build Commands

```bash
# Check Rust backend
cargo check --manifest-path src-tauri/Cargo.toml --all-targets

# Run tests
cargo test --manifest-path src-tauri/Cargo.toml

# Lint
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

# Format check
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check

# Compile without packaging
cargo tauri build --no-bundle

# Build installers
cargo tauri build
```

## Permissions

### macOS

Ghost needs Accessibility permission to observe and replay desktop actions. Keyboard capture requires Input Monitoring.

Enable Ghost in:
```
System Settings → Privacy & Security → Accessibility
System Settings → Privacy & Security → Input Monitoring
```

Then restart the app.

### Windows

Ghost uses Windows-native input hooks and replay APIs. Apps running as administrator or protected system surfaces may not be controllable from a normal user-level Ghost process.

## Known Limitations

- **Coordinate dependency**: Some workflows depend on screen coordinates rather than semantic UI targets. Moving windows may break replay.
- **App compatibility**: Works best with standard macOS/Windows apps. Electron apps, games, and virtualized environments may have limited support.
- **Timing sensitivity**: Fast-changing UIs or network-dependent flows may require manual wait conditions.
- **No mobile support**: iOS and Android are not supported (and likely won't be due to OS restrictions).
- **English-first**: UI element detection works best with English-language apps.

## Contributing

Ghost welcomes contributions focused on:

- Replay reliability improvements
- Semantic target resolution
- Workflow debugging tools
- Safety/policy enhancements
- Documentation and examples

**Not accepting right now:**
- Feature bloat (keep the core narrow and trustworthy)
- Cloud-dependent features (local-first is non-negotiable)
- Autonomous agent modes (user approval required for execution)

See [`docs/`](docs/) for architecture details and product planning.

## License

MIT License — see [LICENSE](LICENSE) for details.

---

**Ghost is boringly trustworthy first, then powerful.** That's how it becomes more impressive than AI assistants that promise magic but deliver anxiety.


