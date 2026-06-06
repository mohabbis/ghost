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

## macOS Gatekeeper note

Without Apple code signing, macOS blocks the app.
Workaround for users: right-click the .app, Open, Open anyway.

To add proper signing, set these GitHub Actions secrets:
- APPLE_CERTIFICATE (base64 .p12)
- APPLE_CERTIFICATE_PASSWORD
- APPLE_ID
- APPLE_PASSWORD
- APPLE_TEAM_ID
