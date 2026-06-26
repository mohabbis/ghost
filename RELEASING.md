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

### First-time notarization runbook

You have an Apple Developer membership, so this is config + secrets — no code
changes. Do it once:

1. **Create the Developer ID Application certificate.**
   - Apple Developer → Certificates, IDs & Profiles → Certificates → **+** →
     *Developer ID Application*. Follow the CSR prompt (Keychain Access →
     Certificate Assistant → *Request a Certificate from a Certificate
     Authority*, "Saved to disk").
   - Download the resulting `.cer`, double-click to add it to your login
     keychain. (It must be your **Developer ID Application** cert, not "Apple
     Development" / "Mac App Distribution" — those won't notarize a DMG.)

2. **Export it as a `.p12`.**
   - Keychain Access → find the cert → expand it so the private key is included
     → right-click → *Export* → `.p12`, set a strong password. That password is
     `P12_PASSWORD`.
   - Base64-encode it for GitHub:

         base64 -i DeveloperID.p12 | pbcopy   # macOS, copies to clipboard

     That value is `BUILD_CERTIFICATE_BASE64`.

3. **Read your signing identity + Team ID.**

         security find-identity -v -p codesigning

   Copy the full string in quotes — e.g. `Developer ID Application: Jane Doe
   (AB12CD34EF)`. That's `APPLE_SIGNING_IDENTITY`; the 10-char code in parens is
   `APPLE_TEAM_ID` (also shown top-right in the Developer portal).

4. **Create an app-specific password for notarytool.**
   - appleid.apple.com → Sign-In & Security → *App-Specific Passwords* → **+** →
     name it "ghost-notarytool". Copy the generated password — that's
     `APPLE_PASSWORD` (NOT your Apple ID login password). `APPLE_ID` is your
     Apple ID email.

5. **Add all six secrets** (next section) and tag a release.

### Secrets to enable notarization

Requires a paid Apple Developer account ($99/yr — a flat annual fee, no
per-build or usage billing, so there is nothing to rate-limit or budget-cap).
Set these in **GitHub → Settings → Secrets and variables → Actions**:

- `BUILD_CERTIFICATE_BASE64` — base64 of your Developer ID Application `.p12`
- `P12_PASSWORD` — password for that `.p12`
- `APPLE_SIGNING_IDENTITY` — e.g. `Developer ID Application: Your Name (TEAMID)`
- `APPLE_ID` — your Apple ID email
- `APPLE_PASSWORD` — an app-specific password (not your Apple ID password)
- `APPLE_TEAM_ID` — your 10-character Team ID

> Bottom line: ad-hoc signing keeps the download working but still shows the
> Gatekeeper dialog. Only notarization removes it. There is no free way around
> this — it requires the paid Apple Developer membership.

### Verifying a notarized build

After the release job finishes, download `Ghost.dmg` on a Mac and confirm the
signature and notarization actually took (don't assume from a green CI run):

```bash
# Gatekeeper should accept it and report a Developer ID source.
spctl -a -vvv -t install Ghost.dmg
# The ticket should be stapled to the DMG / the .app inside.
stapler validate Ghost.dmg
# Inspect the signing identity on the app bundle.
codesign -dvvv /Volumes/Ghost/Ghost.app
```

If `spctl` says "rejected" or `stapler` reports no ticket, the job most likely
fell back to ad-hoc signing because a secret was missing or the certificate was
the wrong type — re-check the five secrets and that the cert is *Developer ID
Application*.

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

> **Cost & billing safety:** Trusted Signing is a flat low monthly fee, but it
> lives in an Azure subscription that *can* accrue charges if other resources
> are created. Before you create anything in Azure, read
> [docs/azure-signing-cost.md](docs/azure-signing-cost.md) — it walks the
> one-time setup and, importantly, how to set budgets/alerts and keep the bill
> bounded so you never get a surprise charge.

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
