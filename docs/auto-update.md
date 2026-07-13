# Auto-update

How Ghost updates itself, what users see, and the one-time setup a maintainer
must complete before auto-update goes live.

Auto-update follows the trust pipeline like every other mutating operation:

```text
Intent (check) -> Plan (update offered) -> Policy (signature verify) -> Approval (Update now) -> Execution (install) -> relaunch
```

Ghost never swaps itself out silently.

## What users experience

1. **On launch**, Ghost makes a single read-only check against the update
   endpoint (`check_for_update`). It downloads nothing and changes nothing.
2. If a newer signed release exists, a dismissible notification appears:
   *"Ghost X.Y.Z is available (you have A.B.C)."* with **Update now** / **Later**.
3. Nothing happens until the user clicks **Update now**. Only then does Ghost
   download the update, **verify its signature against the public key embedded
   in the app**, install it, and relaunch (`install_update`).
4. If verification fails, the update is rejected and nothing is installed.

Failures of the launch check (offline, no manifest, key not configured) are
swallowed in the UI, so a missing/uncut release never blocks startup.

### Platform behavior

| Platform | Downloaded artifact | Install mechanism |
|---|---|---|
| macOS | `Ghost.app.tar.gz` (updater tarball of the `.app`) | in-place bundle swap, relaunch |
| Windows | `Ghost_Setup.exe` (NSIS installer) | runs the installer, relaunch |

macOS ships a universal binary, so `darwin-aarch64` and `darwin-x86_64` point
at the same tarball + signature.

## How the pieces fit together

- **Command surface** — `src-tauri/src/commands/updates.rs`
  (`check_for_update` = read-only/network; `install_update` = user-gated
  network + process replace). Both no-op when the updater is unconfigured.
- **Plugin** — `tauri-plugin-updater`, wired in `src-tauri/src/lib.rs`.
- **Endpoint** — `plugins.updater.endpoints` in `src-tauri/tauri.conf.json`
  points at `releases/latest/download/latest.json`.
- **Public key** — `plugins.updater.pubkey` in `tauri.conf.json`. Verifies
  every downloaded update. Until a real key is set, `check_for_update` returns
  "no update" so nothing is offered.
- **Release manifest** — `latest.json`, published on each GitHub Release,
  lists per-platform download URL + signature.

For auto-update to actually offer an update, a release must publish **all** of:
the installer/tarball, its `.sig`, and a `latest.json` that references them. The
release workflow does this automatically once the signing key secret is set
(see below); with no key it produces the normal installers and skips the
updater artifacts, and the app stays quiet.

## One-time maintainer setup (enables auto-update)

Auto-update stays inert until a maintainer completes these steps **and cuts a
release that embeds the pubkey**. Private key material must stay out of git.

**Status (2026-07-13):** the repo pubkey + GitHub `TAURI_SIGNING_*` secrets are
configured. Existing `v1.2.7` installers were built before the pubkey commit and
will not offer updates; `v1.2.8+` publishes `latest.json` and can update onward.

1. **Generate the updater keypair:**

   ```bash
   cargo tauri signer generate -w ~/.tauri/ghost-updater.key
   ```

2. **Embed the public key** in `src-tauri/tauri.conf.json` →
   `plugins.updater.pubkey` (replace the `REPLACE_WITH_…` placeholder with the
   printed public key). Commit this.

3. **Add the private key as GitHub Actions secrets**
   (Settings → Secrets and variables → Actions):
   - `TAURI_SIGNING_PRIVATE_KEY` — contents of the generated private key
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the password you set (empty if none)

Once the pubkey is committed and the secrets exist, the next tagged release
builds signed updater artifacts and publishes `latest.json`, and installed apps
begin offering the update on launch.

## Cutting a release

See `RELEASING.md`. In short: bump the version in `src-tauri/tauri.conf.json`
and `src-tauri/Cargo.toml` (and `Cargo.lock`), merge to `master`, then tag:

```bash
git tag vX.Y.Z && git push origin vX.Y.Z
```

The tag fires the release workflow, which builds both platforms, publishes the
installers (`Ghost.dmg`, `Ghost_Setup.exe`) the website links to, and — when the
signing key is configured — the updater artifacts and `latest.json`.
