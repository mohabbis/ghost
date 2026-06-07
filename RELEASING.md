# Releasing Ghost

Pushing a version tag triggers the release workflow, which builds
Ghost.dmg (macOS) and Ghost_Setup.exe (Windows) and attaches them
to a GitHub Release. The site download buttons resolve automatically.

## Steps

  1. Make sure master is clean and all PRs are merged
     git checkout master and git pull origin master

  2. Tag the release (use semver)
     git tag v0.1.0

  3. Push the tag — this fires the workflow
     git push origin v0.1.0

GitHub Actions builds both platforms in parallel (~15 min).

## Bumping the version for future releases

Edit both of these before tagging:
- src-tauri/tauri.conf.json  ->  "version"
- src-tauri/Cargo.toml       ->  version in [package]

## macOS Gatekeeper / code signing

The release workflow always signs the macOS app. How it signs depends on
whether Apple Developer secrets are configured:

- **No secrets (default):** the app is **ad-hoc signed** (`APPLE_SIGNING_IDENTITY=-`).
  This prevents "app is damaged" errors but does NOT satisfy Gatekeeper —
  downloaded builds still show "Apple could not verify…" on first launch.
  Users must clear quarantine to run it:

      xattr -dr com.apple.quarantine /Applications/ghost.app

  (or System Settings → Privacy & Security → Open Anyway).

- **With secrets:** the app is signed with your Developer ID **and notarized**,
  so it opens with no prompt. The workflow auto-detects this — just add the
  secrets, no YAML changes needed.

### Secrets to enable notarization

Requires a paid Apple Developer account ($99/yr). Set these in
**GitHub → Settings → Secrets and variables → Actions**:

- `APPLE_CERTIFICATE` — base64 of your Developer ID Application `.p12`
- `APPLE_CERTIFICATE_PASSWORD` — password for that `.p12`
- `APPLE_SIGNING_IDENTITY` — e.g. `Developer ID Application: Your Name (TEAMID)`
- `APPLE_ID` — your Apple ID email
- `APPLE_PASSWORD` — an app-specific password (not your Apple ID password)
- `APPLE_TEAM_ID` — your 10-character Team ID

> Bottom line: ad-hoc signing keeps the download working but still shows the
> Gatekeeper dialog. Only notarization removes it. There is no free way around
> this — it requires the paid Apple Developer membership.
