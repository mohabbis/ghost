# Ghost manual QA checklist

Use this checklist on a **real macOS or Windows machine** with a release build
(prefer [`v1.2.9`](https://github.com/mohabbis/ghost/releases/tag/v1.2.9) or
newer). Automated CI covers Rust logic; this checklist covers desktop UI, OS
permissions, and end-to-end trust behavior.

Record pass/fail, build version, OS version, and notes for each section.

```text
Build: v_____   OS: __________   Tester: __________   Date: __________
```

---

## 0. Preflight

- [ ] Install from release asset (`Ghost.dmg` or `Ghost_Setup.exe`), not `cargo tauri dev`
- [ ] Verify download SHA-256 against [`SHA256SUMS.txt`](https://github.com/mohabbis/ghost/releases/latest/download/SHA256SUMS.txt)
- [ ] Launch app — main shell loads without crash
- [ ] Settings → Diagnostics shows expected version (`1.2.9+`)

---

## 1. Ghost Organizer (core wedge)

Trust pipeline: **Select → Scan → Plan → Review → Approve → Execute → Audit → Undo**

### 1a. Setup

- [ ] Create or select a **test folder** with mixed files (PDFs, images, loose downloads)
- [ ] Create a **Zone** with folder rules (or use default filing preset)
- [ ] Confirm destination boundaries are visible before scan

### 1b. Scan and plan (read-only)

- [ ] Run scan — **no files move** during scan
- [ ] Plan preview lists every proposed move/rename
- [ ] Conflicts and low-confidence items are flagged (not auto-applied)
- [ ] Denied actions appear when policy blocks an operation

### 1c. Approve and execute

- [ ] Approve button is disabled until plan is reviewed
- [ ] After approval, only planned actions run
- [ ] Progress indicator advances per action
- [ ] No silent overwrite — existing destination files are skipped or held
- [ ] No silent delete — delete rules are refused

### 1d. Audit and undo

- [ ] Execution history lists the run with action count
- [ ] Export audit (JSON or CSV) downloads/opens; PII patterns are masked in export only
- [ ] **Undo** restores files to original paths
- [ ] Undo refuses when origin path is occupied or folder is non-empty (best-effort messaging)

### 1e. Crash recovery

- [ ] Force-quit mid-run (optional, destructive) → relaunch
- [ ] Unfinished-run banner appears if a partial run was persisted
- [ ] **Undo** or **Dismiss** resolves the banner correctly

### 1f. Policy pack and audit chain

- [ ] Export policy pack → import on a second machine (or fresh profile)
- [ ] Zones and folder rules round-trip
- [ ] `Verify audit chain` reports `intact` for a completed run

---

## 2. Recording and replay (Routines)

Trust pipeline: **Record → Compress → Review → Policy plan → Approve → Replay → History**

### 2a. Recording

- [ ] Start recording — visible recording state (not silent)
- [ ] Perform 3–5 clicks and a short typed string in a safe app (e.g. TextEdit / Notepad)
- [ ] Stop recording — events appear in workflow list
- [ ] Typed text is redacted or suppressed in review (check secure-field behavior if applicable)

### 2b. Review and compression

- [ ] Open compressed timeline — mouse pairs collapse to `Click` steps
- [ ] Typed runs collapse to `TypeText` (content redacted by default)
- [ ] Ghost Guard audit runs locally (no network) and surfaces risk findings

### 2c. Dry-run and policy

- [ ] Dry-run preview shows per-step intent without executing
- [ ] Routine policy plan shows per-step Allow / Deny / Confirm
- [ ] Replay is blocked until explicit **Approve routine replay**

### 2d. Replay execution

- [ ] Replay respects pacing and can be paused/cancelled
- [ ] Live step counter updates during replay
- [ ] Replay history records success/failure and resolution trace
- [ ] Coordinate-fallback steps are flagged in history when semantic match fails

### 2e. Known limitations (expect these)

- [ ] Routine replay has **no undo/vault** yet — only Organizer undo is durable
- [ ] Wrong focused window can still cause misfires — verify target app is frontmost

---

## 3. Permissions and diagnostics

### macOS

- [ ] Accessibility permission prompt or Settings link works
- [ ] Input Monitoring permission (if prompted) can be granted
- [ ] Diagnostics reflects granted/denied state accurately

### Windows

- [ ] App launches without elevation unless user chooses it
- [ ] Replay/input works in a standard user session

### All platforms

- [ ] Settings → Diagnostics → export telemetry (local file only)
- [ ] Organizer time-to-value milestones populate after first zone/plan/run/undo

---

## 4. Local auth and vault

- [ ] Set local vault password — workflows/organizer data encrypt at rest
- [ ] Restart app — password unlock works
- [ ] Wrong password is rejected without data loss

---

## 5. Account sign-in (optional)

Requires operator-configured OAuth client IDs.

- [ ] Sign in with Microsoft opens system browser consent
- [ ] Sign in with Google opens system browser consent
- [ ] Profile (email/name) appears in Settings after success
- [ ] Sign out clears identity without deleting local workflows/organizer data

---

## 6. Auto-updater (v1.2.9+ only)

- [ ] App launch performs silent **check only** (no download without approval)
- [ ] When no newer release exists, no update nag appears
- [ ] When a newer signed release exists, dismissible notification appears
- [ ] **Update now** downloads, verifies signature, installs, and relaunches
- [ ] Failed signature verification rejects install (no silent downgrade)

> Installers older than `v1.2.9` may not offer updates until manually upgraded once.

---

## 7. Experimental features (off in stock build)

Enable only with `--features experimental` build or documented dev flag.

- [ ] Stock build: AI Providers, Power BI, MCP sections are **hidden**
- [ ] Experimental build: sections appear after `is_experimental_enabled` is true
- [ ] Power BI export requires preview before push
- [ ] No experimental command runs without gating check (see `ipc_contract` tests)

---

## 8. Marketing / demo accuracy

- [ ] Guard Desk + POS Bridge behaves as **simulation/demo**, not certified compliance
- [ ] Site/README version matches installed build and [latest GitHub release](https://github.com/mohabbis/ghost/releases/latest)
- [ ] Marketing copy says **no account required** for core use (not "no accounts exist")
- [ ] Sign-in is in desktop app **Settings** only — not on the marketing site
- [ ] "Files never leave machine" holds for Organizer path (experimental cloud/AI may use network when enabled)

---

## 9. Regression smoke (5 minutes)

Quick re-check after any release candidate:

1. Organizer: scan → approve small plan → undo
2. Record 2 clicks → compress → dry-run → policy approve → replay
3. Verify audit chain on last Organizer run
4. Launch check: app starts clean, no updater error modal

---

## Automated counterpart

Before manual QA, ensure CI is green:

```bash
make ci
cargo test --manifest-path src-tauri/Cargo.toml --features experimental  # when touching experimental code
```

Linux CI uses the headless backend; it does **not** replace sections 2–6 above.
