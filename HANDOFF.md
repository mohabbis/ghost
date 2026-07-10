# Ghost Project — Detailed Handoff Prompt for Continued Development

**Last Updated:** 2026-07-10
**Status:** `master` green. **v1.2.5 released** (`Ghost.dmg` + `Ghost_Setup.exe`). Professional polish in progress: atomic release publish + checksums, marketing honesty, app error/version UX.

---

## What You're Picking Up

You are inheriting **Ghost**, a Tauri (Rust + vanilla JS) local-first desktop automation product for macOS and Windows. Read `AGENTS.md` and `CLAUDE.md` first — they are the canonical contract. The product promise is trustworthy execution:

```text
Record -> Inspect -> Approve -> Replay -> Audit -> Undo
```

The current wedge is **Ghost Organizer** (safe file organization: scan → plan → review → approve → move/rename → audit → undo), fully wired end to end through the policy engine, executor, audit log, and undo journal.

---

## Recent Changes (through v1.2.4 + polish)

1. **v1.2.4:** ID-scan OCR hardening, DOM contract test, signing-docs fix, version bump.
2. **Release pipeline:** single publish job after both platform builds; `SHA256SUMS.txt`; optional updater artifacts / `latest.json` when keys exist; no more Mac-only or Windows-only partial releases.
3. **Product polish:** platform-neutral copy, honest signing/preview language, readable IPC error toasts, Settings shows app version.

---

## Current Health

- Rust CI + Deploy Website on `master`.
- Validation: `make ci` (fmt + clippy + test), `make build`, `make dev`.
- Local Linux needs GTK/webkit deps in `AGENTS.md` plus `libssl-dev` / `pkg-config`.

---

## Immediate Next Steps

1. **Notarization / Windows signing secrets** so releases stop being ad-hoc/unsigned. See `RELEASING.md`.
2. **Updater pubkey:** replace `REPLACE_WITH_…` in `tauri.conf.json` and set `TAURI_SIGNING_PRIVATE_KEY` so `latest.json` publishes automatically.
3. **Keep Guard Desk / POS Bridge suggestion-only** — never auto-execute from ID-scan output.
4. Continue `AGENTS.md` build order: Organizer polish → replay reliability → release quality → AI last (gated).

---

## Known Risks

- Without Apple/Azure secrets, releases are preview-quality (ad-hoc macOS / unsigned Windows).
- `parse_id_document` handles PII — keep local and suggestion-only.
- Experimental commands stay behind `--features experimental`; CI does not run that leg.
