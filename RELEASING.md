# Releasing Ghost

Pushing a `v*` tag triggers `.github/workflows/release.yml`, which:

1. Builds **macOS** (`Ghost.dmg`) and **Windows** (`Ghost_Setup.exe`) in parallel
2. Waits for **both** builds to succeed
3. Publishes **one** GitHub Release with installers, `SHA256SUMS.txt`, and
   (when updater signing keys are configured) updater artifacts + `latest.json`

The marketing site download buttons resolve
`releases/latest/download/{Ghost.dmg,Ghost_Setup.exe}` automatically.

## Steps

1. Make sure `master` is clean and the version bump PR is merged:

       git checkout master && git pull origin master

2. Bump the version in **all** of these (keep them identical):
   - `src-tauri/Cargo.toml` → `[package] version`
   - `src-tauri/tauri.conf.json` → `"version"`
   - `src-tauri/Cargo.lock` (the `name = "ghost"` entry)
   - Marketing site strings in `public/index.html` and `public/main.js`
   - `README.md` current-version line (if present)

   macOS releases automatically compile **GhostAXHelper** via
   `scripts/build-ghost-ax-helper.sh` and bundle it with
   `--config '{"bundle":{"externalBin":["bin/ghost-ax-helper"]}}'` before
   `cargo tauri build` (keeps Linux/Windows CI free of macOS-only sidecars).

3. Tag and push (semver):

       git tag v1.2.7
       git push origin v1.2.7

GitHub Actions builds both platforms (~20 min). The publish job fails closed if
either platform is missing — you will not get a one-sided release.

### Re-running a release

Use **Actions → Release → Run workflow** and pass the existing tag (e.g.
`v1.2.7`). The publish job re-attaches assets to that tag.

## What each release attaches

| File | Always? | Purpose |
|---|---|---|
| `Ghost.dmg` | yes | Universal macOS installer |
| `Ghost_Setup.exe` | yes | Windows NSIS installer |
| `SHA256SUMS.txt` | yes | SHA-256 digests for verification |
| `*.app.tar.gz` + `.sig` | if updater key set | macOS auto-update payload |
| `*-setup.exe.sig` | if updater key set | Windows auto-update signature |
| `latest.json` | if both updater sigs exist | Updater manifest |

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
# Verify the published checksum.
shasum -a 256 -c SHA256SUMS.txt
```

If `spctl` says "rejected" or `stapler` reports no ticket, the job most likely
fell back to ad-hoc signing because a secret was missing or the certificate was
the wrong type — re-check the secrets and that the cert is *Developer ID
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
`.app.tar.gz`/`.sig` (macOS) and `-setup.exe.sig` (Windows). The macOS job
must build `--bundles app,dmg` — `dmg` alone is not an updater-enabled target
and will skip signatures. The publish job assembles `latest.json` automatically
when both platform signatures are present.

Until the pubkey placeholder is replaced and the private key secret is set,
auto-update stays inactive — installers still publish normally.
