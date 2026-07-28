# Verifying Ghost downloads

Every Ghost release ships with integrity and provenance material so you can
check what you downloaded before running it. Three independent layers apply:

| Layer | What it proves | Artifact |
|---|---|---|
| SHA-256 checksums | The bytes you downloaded are the bytes the release published | `SHA256SUMS.txt` |
| Cosign keyless signatures | The artifact was built by this repository's GitHub release workflow (not re-uploaded by a third party) | `<file>.cosign.sig` + `<file>.cosign.pem` |
| Minisign updater signatures | The in-app auto-updater only installs updates signed with the key pinned inside the app | `*.app.tar.gz.sig`, `*-setup.exe.sig`, `latest.json` |

Platform code signing (macOS Developer ID + notarization, Windows
Authenticode) applies additionally when release signing secrets are
configured — see [`RELEASING.md`](../RELEASING.md).

## 1. Verify checksums

Download the installer(s) and `SHA256SUMS.txt` from the same release into one
directory.

macOS / Linux:

```bash
shasum -a 256 -c SHA256SUMS.txt --ignore-missing
```

Or check a single file and compare against the hash table in the release notes:

```bash
shasum -a 256 Ghost.dmg
```

Windows (PowerShell):

```powershell
Get-FileHash .\Ghost_Setup.exe -Algorithm SHA256
# Compare the printed hash against the Ghost_Setup.exe row in SHA256SUMS.txt
# or the table in the release notes.
```

## 2. Verify provenance (cosign)

Release artifacts are signed keylessly via [Sigstore
cosign](https://docs.sigstore.dev/): the release workflow exchanges its GitHub
OIDC identity for a short-lived certificate, so a valid signature proves the
artifact came out of this repository's release pipeline. There is no
maintainer-held signing key to leak or rotate.

Install cosign, download the artifact plus its `.cosign.sig` and
`.cosign.pem`, then:

```bash
cosign verify-blob \
  --certificate Ghost.dmg.cosign.pem \
  --signature Ghost.dmg.cosign.sig \
  --certificate-identity-regexp 'github.com/mohabbis/ghost' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  Ghost.dmg
```

`Verified OK` means the file was signed by a GitHub Actions run of this
repository. The same command works for `Ghost_Setup.exe` and
`SHA256SUMS.txt` (verifying the checksum file itself closes the loop: a
tampered `SHA256SUMS.txt` cannot carry a valid signature).

## 3. Auto-update path

The in-app updater does not rely on either mechanism above: update packages
are verified against a minisign public key pinned in the app
(`tauri.conf.json` → `plugins.updater.pubkey`) before installation. If a
release lacks updater signatures, the updater stays inert rather than
installing unverified bytes.

## Scope notes

- Signatures are produced only by tagged release builds; ad-hoc or preview
  artifacts may ship without them (the release notes state the signing mode).
- Windows Authenticode signing is not yet configured; SmartScreen may warn on
  first run. Checksum + cosign verification above is the interim integrity
  path. See `RELEASING.md`.
