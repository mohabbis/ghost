# Ghost Project — Detailed Handoff Prompt for Continued Development

**Last Updated:** 2026-07-10
**Status:** `master` green (Rust CI + Deploy Website). **v1.2.3 released**. This branch prepares **v1.2.4** (ID-scan OCR hardening, DOM contract test, signing-docs fix, version bump).

---

## 🎯 What You're Picking Up

You are inheriting **Ghost**, a Tauri (Rust + vanilla JS) local-first desktop automation product for macOS and Windows. Read `AGENTS.md` and `CLAUDE.md` first — they are the canonical contract. The product promise is trustworthy execution:

```text
Record -> Inspect -> Approve -> Replay -> Audit -> Undo
```

The current wedge is **Ghost Organizer** (safe file organization: scan → plan → review → approve → move/rename → audit → undo), fully wired end to end through the policy engine, executor, audit log, and undo journal.

---

## 📁 Recent Changes (through v1.2.4)

1. **Release v1.2.4 (this session):**
   * Bumped `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.lock`, and marketing site strings to `1.2.4`.
   * Landed unmerged cursor follow-ups: ID-scan OCR noise hardening (`reorder_name`, `normalize_digits`, separator-tolerant dates) + `frontend_dom_contract` test + macOS signing checklist secret-name fix.
2. **Release v1.2.3:**
   * First signed macOS build path (secrets-dependent); signing secret-name fallback in `release.yml`.
3. **Release v1.2.2 + marketing + ID scanning:**
   * Working `Ghost.dmg` + `Ghost_Setup.exe`; Guard Desk `parse_id_document` + local OCR; light marketing site.

---

## ✅ Current Health

- `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, and `cargo test` should pass on this branch before merge.
- Rust CI green on `master`; Deploy Website (Vercel) green.
- Local Linux build/test needs the GTK/webkit deps in `AGENTS.md` **plus** `libssl-dev` + `pkg-config` (openssl-sys).

---

## 🚀 Immediate Next Steps

1. **Merge this PR and tag `v1.2.4`** on `master` to fire `release.yml` (builds DMG + NSIS and publishes the GitHub Release). Site download buttons already resolve `releases/latest`.
2. **Release signing/notarization:** confirm Apple/Windows secrets are set so 1.2.4 is not ad-hoc. See `RELEASING.md`, `docs/macos-signing-checklist.md`.
3. **`parse_id_document` risk hardening:** keep Guard Desk output suggestion-only (never auto-execute); consider an explicit user-enabled toggle for PII parsing.
4. **Continue the build order in `AGENTS.md`:** policy/Zones polish → Organizer polish → replay reliability → release quality → AI suggestions last (gated).

---

## ⚠️ Known Risks / Notes

- Releases are **not signed/notarized** without secrets — do not claim production readiness.
- `parse_id_document` handles personal data; keep it local, suggestion-only, and out of any auto-execute path.
- Experimental commands stay behind the `experimental` Cargo feature; CI does not run that leg.
- Validation: `make ci`, `make build`, `make dev`.
