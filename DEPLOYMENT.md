# Ghost Deployment Guide

## Project structure

```
ghost/
├── src/                    # Tauri desktop app frontend (vanilla JS/HTML/CSS)
├── public/                 # Marketing site (ghost.muharafiq.com) — separate from src/
├── src-tauri/              # Rust backend + tauri.conf.json
├── docs/                   # Operational docs
└── .github/workflows/
    ├── rust.yml            # CI (check, test, clippy, fmt, smoke build)
    ├── release.yml         # Tag-triggered DMG + NSIS + SHA256SUMS publish
    └── deploy-website.yml  # Vercel deploy for public/ → ghost.muharafiq.com
```

`src/` and `public/` are **not** identical: `src/` is the desktop app UI;
`public/` is the marketing/download site.

## Desktop application

**Platforms:** macOS and Windows  
**Distribution:** GitHub Releases  
**Workflow:** `.github/workflows/release.yml`

### Release process

1. Bump version in `Cargo.toml`, `tauri.conf.json`, `Cargo.lock`, and `public/`
   version strings (see `RELEASING.md`).
2. Tag and push:

   ```bash
   git tag v1.2.5
   git push origin v1.2.5
   ```

3. CI builds both platforms, then a single publish job attaches:
   - `Ghost.dmg`
   - `Ghost_Setup.exe`
   - `SHA256SUMS.txt`
   - updater artifacts + `latest.json` when signing keys are configured

### Download links

- macOS: `https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg`
- Windows: `https://github.com/mohabbis/ghost/releases/latest/download/Ghost_Setup.exe`
- Checksums: `https://github.com/mohabbis/ghost/releases/latest/download/SHA256SUMS.txt`

## Marketing website

**Domain:** [ghost.muharafiq.com](https://ghost.muharafiq.com)  
**Source:** `public/`  
**Hosting:** Vercel via `.github/workflows/deploy-website.yml`

Pushes that touch `public/**` on `master` deploy automatically. The workflow
HTTP-checks that `releases/latest` download URLs return 200 before deploying.

Required secrets: `VERCEL_TOKEN`, `VERCEL_ORG_ID`, `VERCEL_PROJECT_ID`.

## Continuous integration

**Workflow:** `.github/workflows/rust.yml`  
**Triggers:** push/PR to `main` / `master` / `develop`

Runs fmt, check, test, clippy, and a `cargo tauri build --no-bundle` smoke test
on macOS/Windows. Experimental features are **not** exercised in CI — validate
locally with `--features experimental` when touching that code.

## Checklist before a public tag

- [ ] Version bumped in Cargo.toml, tauri.conf.json, Cargo.lock, public/
- [ ] `make ci` (or equivalent) green on the release commit
- [ ] No marketing copy promising notarization/signing you have not configured
- [ ] Tag pushed; Release workflow publish job green
- [ ] Spot-check `SHA256SUMS.txt` and both installers on the GitHub Release
