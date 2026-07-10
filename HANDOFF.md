# Ghost Project — Detailed Handoff Prompt for Continued Development

**Last Updated:** 2026-07-10
**Status:** `master` green (Rust CI + Deploy Website). **v1.2.2 released** with working `Ghost.dmg` + `Ghost_Setup.exe`. PRs #135 (marketing), #136 (release bump), #137 (ID scanning), #138 (asset cleanup + Guard Desk wiring) all merged.

---

## 🎯 What You're Picking Up

You are inheriting **Ghost**, a Tauri (Rust + vanilla JS) local-first desktop automation product for macOS and Windows. Read `AGENTS.md` and `CLAUDE.md` first — they are the canonical contract. The product promise is trustworthy execution:

```text
Record -> Inspect -> Approve -> Replay -> Audit -> Undo
```

The current wedge is **Ghost Organizer** (safe file organization: scan → plan → review → approve → move/rename → audit → undo), fully wired end to end through the policy engine, executor, audit log, and undo journal.

---

## 📁 Recent Changes (through v1.2.2)

1. **Release v1.2.2 (this session):**
   * Bumped `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.lock` to `1.2.2` (PR #136).
   * Pushed the `v1.2.2` tag; the `release.yml` workflow built and published the universal macOS DMG and Windows NSIS installer. Assets: `Ghost.dmg` (~16 MB), `Ghost_Setup.exe` (~7 MB).
   * Signing is ad-hoc unless Apple/Windows signing secrets are configured — this is a **preview** release, not notarized.
2. **Marketing site redesign (PR #135):**
   * Light/white professional theme (teal accent, IBM Plex + Newsreader), one-liner **"Approve before it acts."**
   * Four interactive demo tabs: Organizer, Record→Replay, Client filing, and **Guard Desk → Approve → POS Bridge**.
   * Download buttons point to `github.com/mohabbis/ghost/releases/latest/download/{Ghost.dmg,Ghost_Setup.exe}` (not repo-committed binaries). Site version strings read `v1.2.2`.
3. **ID scanning (PRs #137 + #138):**
   * `src-tauri/src/core/id_scan.rs` — deterministic parser that turns already-OCR'd text into structured ID-document fields (name, ID number, DOB, expiry) plus derived signals (age, expiry state, review flags). Pure text in / struct out — no image, IO, or network. 14 unit tests.
   * `parse_id_document` command in stable `commands/core.rs`, registered in `lib.rs`, wired into the Guard Desk UI. Complements the existing `run_ocr_on_image` (local macOS Vision / Windows OCR).
   * Documented in `docs/command-registry.md` (risk `low`) and `docs/core-boundaries.md` (stable-core, sensitive fields stay local).
4. **Asset/repo hygiene (PR #138):**
   * Removed the ~14 MB `public/downloads/*.{dmg,exe}` binaries from git (the site links to GitHub releases instead) and stray `public/assets/*` scaffolding.

---

## ✅ Current Health

- `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, and `cargo test` (~392 unit + ipc_contract + resolution_benchmark) all pass locally on `master`.
- Rust CI green on `master`; Deploy Website (Vercel) green.
- Local Linux build/test needs the GTK/webkit deps in `AGENTS.md` **plus** `libssl-dev` + `pkg-config` (openssl-sys); CI installs the GTK set — add libssl if you extend the CI image.

---

## 🚀 Immediate Next Steps

1. **`parse_id_document` risk hardening (small):** it currently lives in stable `core.rs` and is registered unconditionally. Confirm the Guard Desk UI never lets its output auto-execute anything (it must stay a suggestion feeding the approve step), and consider whether the ID-parsing surface belongs behind an explicit user-enabled toggle given it handles PII.
2. **Release signing/notarization:** wire the Apple signing secrets (`BUILD_CERTIFICATE_BASE64`, `P12_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`) and Windows signing so future releases stop being ad-hoc. See `RELEASING.md`, `docs/macos-signing-checklist.md`, `docs/azure-signing-cost.md`.
3. **Verify the live download path:** after `v1.2.2`, confirm `releases/latest/download/Ghost.dmg` and `Ghost_Setup.exe` resolve from the deployed site (they should, since assets are published and not draft).
4. **Guard the multi-agent workflow:** this repo takes many concurrent PRs. Keep PRs small and single-purpose, `git fetch origin master` + rebase before pushing, and add one line to the `generate_handler!` list / `src/main.js` without reflowing neighbors (both are conflict hotspots). PR #138 was a grab-bag that merged cleanly this time but is the anti-pattern to avoid.
5. **Continue the build order in `AGENTS.md`:** policy primitives → Zones/folder rules → Organizer planner/preview/executor polish → replay reliability/semantic targeting → release quality → AI suggestions last (suggestion-only, gated).

---

## ⚠️ Known Risks / Notes

- Releases are **not signed/notarized** without secrets — do not claim production readiness.
- `parse_id_document` handles personal data; keep it local, suggestion-only, and out of any auto-execute path.
- Experimental commands (`commands/experimental.rs`) stay behind the `experimental` Cargo feature (off by default); CI does not run the experimental leg — validate those locally with `--features experimental`.
- Validation commands: `make ci` (fmt-check + clippy + test), `make build` (`cargo tauri build --no-bundle`), `make dev`.
