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

- `BUILD_CERTIFICATE_BASE64` — base64 of your Developer ID Application `.p12`
- `P12_PASSWORD` — password for that `.p12`
- `APPLE_SIGNING_IDENTITY` — e.g. `Developer ID Application: Your Name (TEAMID)`
- `APPLE_ID` — your Apple ID email
- `APPLE_PASSWORD` — an app-specific password (not your Apple ID password)
- `APPLE_TEAM_ID` — your 10-character Team ID

> Bottom line: ad-hoc signing keeps the download working but still shows the
> Gatekeeper dialog. Only notarization removes it. There is no free way around
> this — it requires the paid Apple Developer membership.

## Windows code signing (Azure Trusted Signing)

Unsigned Windows installers trigger SmartScreen ("Windows protected your PC")
and an "Unknown publisher" UAC prompt. The release workflow signs the installer
**during the build** (via Tauri's `signCommand`, so the updater signature is
computed over the already-signed file) using Azure Trusted Signing — but only
when the Azure secrets are present. With no secrets it falls back to the
previous unsigned build, so nothing breaks before setup is complete.

### One-time Azure setup

1. Create an **Azure Trusted Signing** account + a **certificate profile**
   (identity validation required; individual accounts are supported).
2. Create an **Entra service principal** and grant it the *Trusted Signing
   Certificate Profile Signer* role on the account.

### Secrets to enable Windows signing

Set in **GitHub → Settings → Secrets and variables → Actions**:

- `AZURE_TENANT_ID`, `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET` — the service
  principal credentials (used by `trusted-signing-cli` via DefaultAzureCredential).
- `AZURE_TS_ENDPOINT` — regional endpoint, e.g. `https://eus.codesigning.azure.net/`.
- `AZURE_TS_ACCOUNT` — your Trusted Signing account name.
- `AZURE_TS_PROFILE` — your certificate profile name.

## Auto-update (signed)

Ghost ships a signed auto-updater (`tauri-plugin-updater`). On launch it checks
`releases/latest/download/latest.json`, and installs **only after the user
approves** (the install verifies the update signature against the public key
embedded in `tauri.conf.json`).

### One-time updater setup

1. Generate the updater keypair:

       cargo tauri signer generate -w ~/.tauri/ghost-updater.key

2. Put the **public** key into `src-tauri/tauri.conf.json` →
   `plugins.updater.pubkey` (replace the `REPLACE_WITH_…` placeholder).
3. Add the **private** key + password as GitHub secrets:
   - `TAURI_SIGNING_PRIVATE_KEY`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

When `TAURI_SIGNING_PRIVATE_KEY` is set, both build jobs pass
`--config '{"bundle":{"createUpdaterArtifacts":true}}'`, producing the
`.app.tar.gz`/`.sig` (macOS) and `-setup.exe.sig` (Windows) the workflow
uploads to the release.

### Generating `latest.json`

The updater needs a `latest.json` manifest published on the release listing each
platform's download URL + signature. The robust way to generate it is to migrate
`release.yml` to **`tauri-apps/tauri-action`**, which emits the manifest
automatically. Until then, assemble it by hand from the uploaded `.sig` files:

```json
{
  "version": "1.0.12",
  "notes": "See the release notes.",
  "pub_date": "2026-06-25T00:00:00Z",
  "platforms": {
    "darwin-aarch64": { "signature": "<contents of Ghost.app.tar.gz.sig>", "url": "https://github.com/mohabbis/ghost/releases/download/v1.0.12/Ghost.app.tar.gz" },
    "darwin-x86_64":  { "signature": "<same .sig>",                         "url": "https://github.com/mohabbis/ghost/releases/download/v1.0.12/Ghost.app.tar.gz" },
    "windows-x86_64": { "signature": "<contents of *-setup.exe.sig>",       "url": "https://github.com/mohabbis/ghost/releases/download/v1.0.12/Ghost_Setup.exe" }
  }
}
```

Upload that file to the release as `latest.json`.
