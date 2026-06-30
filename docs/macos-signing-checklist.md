# macOS signing & notarization — one-page checklist

A focused, do-it-once runbook for turning the macOS release from an ad-hoc
build (which still trips Gatekeeper) into a **Developer ID–signed, notarized**
build that opens with no warning. This condenses the macOS portion of
[`RELEASING.md`](../RELEASING.md) into an ordered checklist.

> **Scope:** macOS only. It is fully independent of Windows signing (Azure
> Trusted Signing) and needs **no workflow-file changes** — `release.yml`
> auto-detects the secrets and switches from ad-hoc to full signing on its own.
> You do **not** need the GitHub `workflow` scope for any of this.

## Pre-flight — already verified in this repo

These are confirmed in `main`/`master`, so you only have to do the account side:

- `release.yml` reads exactly the six secret names below (no name mismatch).
- `src-tauri/Ghost.entitlements` exists and is hardened-runtime-compatible
  (minimal: only `com.apple.security.automation.apple-events`).
- `src-tauri/icons/icon.icns` is present; bundle `identifier` is set
  (`com.muhammadrafiq.ghost`).
- `tauri.conf.json` and `Cargo.toml` versions match.
- No signing identity is hardcoded — it comes from the injected env vars.

## What you need

- A paid **Apple Developer** membership ($99/yr flat — no per-build billing).
- A Mac (to create/export the certificate and to verify the result).

## The six GitHub secrets

Set under **GitHub → repo Settings → Secrets and variables → Actions**:

| Secret | What it is |
| --- | --- |
| `BUILD_CERTIFICATE_BASE64` | base64 of your Developer ID Application `.p12` |
| `P12_PASSWORD` | the password you set when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Your Name (AB12CD34EF)` |
| `APPLE_TEAM_ID` | your 10-character Team ID (the code in the parens) |
| `APPLE_ID` | your Apple ID email |
| `APPLE_PASSWORD` | an **app-specific** password (NOT your Apple ID login password) |

## Steps

### A. Certificate → `BUILD_CERTIFICATE_BASE64`, `P12_PASSWORD`

1. Apple Developer → Certificates, IDs & Profiles → Certificates → **+** →
   **Developer ID Application**. Use the CSR prompt (Keychain Access →
   Certificate Assistant → *Request a Certificate from a Certificate
   Authority*, "Saved to disk").
   - ⚠️ It must be **Developer ID Application** — *not* "Apple Development" or
     "Mac App Distribution". Those cannot notarize a DMG.
2. Download the `.cer`, double-click to add it to your **login** keychain.
3. Keychain Access → find the cert → **expand it so the private key is
   included** → right-click → **Export** → `.p12`, set a strong password.
   - That password is **`P12_PASSWORD`**.
   - Base64-encode the file (this value is **`BUILD_CERTIFICATE_BASE64`**):

     ```bash
     base64 -i DeveloperID.p12 | pbcopy   # macOS, copies to clipboard
     ```

### B. Identity + Team ID → `APPLE_SIGNING_IDENTITY`, `APPLE_TEAM_ID`

```bash
security find-identity -v -p codesigning
```

- Copy the full quoted string, e.g. `Developer ID Application: Jane Doe
  (AB12CD34EF)` → **`APPLE_SIGNING_IDENTITY`**.
- The 10-char code in parens → **`APPLE_TEAM_ID`** (also shown top-right in the
  Developer portal).

### C. Notarization login → `APPLE_ID`, `APPLE_PASSWORD`

1. appleid.apple.com → Sign-In & Security → **App-Specific Passwords** → **+** →
   name it "ghost-notarytool". Copy the generated password → **`APPLE_PASSWORD`**.
2. Your Apple ID email → **`APPLE_ID`**.

### D. Add all six secrets

GitHub → repo **Settings → Secrets and variables → Actions → New repository
secret**, once per secret above.

### E. Release

1. Bump the version if needed in **both** `src-tauri/tauri.conf.json`
   (`"version"`) and `src-tauri/Cargo.toml` (`[package] version`).
2. Tag and push:

   ```bash
   git tag v1.0.12
   git push origin v1.0.12
   ```

The macOS job detects the secrets and switches from ad-hoc to full Developer-ID
signing + notarization automatically — no YAML change required.

### F. Verify (don't trust a green check — confirm on a Mac)

```bash
spctl -a -vvv -t install Ghost.dmg      # expect "accepted", source=Developer ID
stapler validate Ghost.dmg              # expect a stapled ticket
codesign -dvvv /Volumes/Ghost/Ghost.app # expect your Developer ID identity
```

If `spctl` says **rejected** or `stapler` reports no ticket, the job almost
certainly fell back to ad-hoc signing because a secret was missing or the
certificate was the wrong type. Re-check the six secrets and that the cert is
**Developer ID Application**.

## Notes

- **Lead time:** the certificate is the only step with any delay — create it
  early even if you plan to release later.
- **Why notarization matters here specifically:** Accessibility / Input
  Monitoring grants (which a recorder app needs) only *persist across updates*
  when the app keeps a stable Developer ID signature. Ad-hoc builds lose those
  grants on every update.
- **Windows / auto-updater** are separate and intentionally out of scope here.
  See `RELEASING.md` for the Azure Trusted Signing and updater-keypair steps
  when you choose to tackle them.
