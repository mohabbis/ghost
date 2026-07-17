# Product Hunt / public-download pre-launch checklist

Use this before cutting (or advertising) a public download for Product Hunt,
YC demo day, or similar. It is an **operational gate**, not a claim that the
current build is production-ready.

**As of 2026-07-17 (post-#291 on `master`):**

| Layer | State |
|---|---|
| Source tree (`Cargo.toml` / `tauri.conf.json`) | **2.0.4** (unreleased) |
| Latest GitHub Release | **[v2.0.3](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)** (2026-07-15) |
| Site / README advertised version | **v2.0.3** (intentional trail — see `#284`) |
| #291 (verification receipts in Replay History) | Merged on `master`; **not** in the published DMG/EXE until a new tag |

Do **not** bump the source version for this checklist. If the PH download must
include #291+, tag and release the existing **2.0.4** after the gates below —
do not advertise 2.0.4 on the site until that tag's assets exist.

---

## Verdict template (fill before launch)

```text
PH download build:     v_____   (must match GitHub Release + site + README)
macOS Gatekeeper:      [ ] verified on a clean Mac  /  [ ] CI-claimed only
Windows SmartScreen:   [ ] signed  /  [x] unsigned (disclose + walkthrough)
Site deploy:           [ ] live copy matches tagged release
Demo script:           [ ] recorded walkthrough on release bits
Honest-copy pass:      [ ] no overclaims
```

Ship only when every required box in §1–§5 is checked or explicitly deferred
with public disclosure.

---

## 1. Version & download integrity (required)

- [ ] Decide the PH asset: **reuse v2.0.3** *or* **cut v2.0.4** from current
      `master` (needed if the launch demo depends on #291 receipts UI).
- [ ] `public/index.html`, `public/main.js`, and `README.md` all advertise the
      **same** tag that `releases/latest` resolves to — never the unreleased
      source version alone.
- [ ] `https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg`
      and `…/Ghost_Setup.exe` return HTTP 200 (site deploy workflow also
      checks this).
- [ ] Download both installers + `SHA256SUMS.txt`; verify:

      ```bash
      shasum -a 256 -c SHA256SUMS.txt --ignore-missing
      ```

- [ ] Release notes list macOS signing mode and Windows signing mode honestly
      (see published v2.0.3: macOS **full** / notarization **enabled**; Windows
      **unsigned**).

> Updater artifacts (`latest.json`, `.sig` files) are separate from platform
> code signing. Their presence does **not** mean Authenticode or Gatekeeper
> acceptance.

---

## 2. macOS notarization (required for a frictionless Mac download)

CI claiming `SIGNING_MODE=full` / `notarization=enabled` is necessary but
**not sufficient**. Per `RELEASING.md`, verify on a real Mac from the published
DMG:

```bash
spctl -a -vvv -t install Ghost.dmg
stapler validate Ghost.dmg
# after mounting:
codesign -dvvv /Volumes/Ghost/Ghost.app
```

- [ ] `spctl` accepts with a Developer ID source
- [ ] `stapler validate` reports a stapled ticket
- [ ] First-launch on a clean Mac opens **without** quarantine / "Apple could
      not verify…" workarounds
- [ ] GhostAXHelper is present at
      `Ghost.app/Contents/MacOS/ghost-ax-helper` (v2.0.3 release notes claim
      bundled — confirm on the DMG you will link)

If any check fails, treat the build as **ad-hoc / not launch-ready** for Mac
PH traffic until secrets/cert are fixed and a new tag is cut. Runbook:
`RELEASING.md` + `docs/macos-signing-checklist.md`.

---

## 3. Windows Authenticode (blocker for "clean" Win download)

**Current public fact:** Windows installers ship **unsigned**. SmartScreen /
"Unknown publisher" is expected. Azure Trusted Signing is documented but
unconfigured (`docs/azure-signing-cost.md`, `docs/windows-signing-checklist.md`).

Choose one before launch:

| Path | Action |
|---|---|
| **A — Launch with disclosure** (allowed) | Keep site copy: "Windows unsigned (SmartScreen may warn)". Add a one-screen PH/FAQ: checksums + "More info → Run anyway". Do **not** claim signed. |
| **B — Block Win until signed** (preferred for trust-sensitive launch) | Configure Azure Trusted Signing secrets, cut a new release, verify publisher name on the EXE, then update site copy. |

- [ ] Path A or B chosen explicitly
- [ ] If Path A: PH maker comment / FAQ includes the SmartScreen steps
- [ ] If Path B: signed EXE verified; site no longer says "unsigned"

---

## 4. Website & marketing honesty (required)

Live site: `https://ghost.muharafiq.com/` (Vercel via `deploy-website.yml`).

- [ ] Latest `Deploy Website` run on `master` succeeded after the last
      `public/**` change
- [ ] Hero / download section version matches the GitHub Release tag
- [ ] Download buttons still point at `releases/latest/download/…`
- [ ] Copy still labels the build a **technical preview** (or equivalent)
- [ ] No claim of full source↔destination reconciliation (roadmap only —
      per-step verification is what ships)
- [ ] Guard Desk / POS Bridge stays **prototype / not certified compliance**
- [ ] No "autonomous agent" / silent-control framing (`AGENTS.md` rule 10)

---

## 5. Demo script gaps (required for PH maker video)

Canonical demo: `docs/GHOST_2_DEMO.md` (invoice → Finance Action Plan).

- [ ] Run on the **same** tagged installers PH will download (not `cargo tauri
      dev`)
- [ ] macOS: Accessibility granted; TextEdit log uses AX `set_value` (not
      coordinate/enigo fallback) when claiming semantic replay
- [ ] Organizer path: Zone → Scan → Review → Approve → Execute → receipt → Undo
- [ ] After #291: reopen **Replay History → Verification receipts** and show
      persisted Verified / Mismatch chips (only if the tagged build includes
      that commit)
- [ ] Emergency stop / cancel during replay demonstrated once
- [ ] Screen walkthrough recorded for the launch post
- [ ] Manual QA minimum from `docs/manual-qa-checklist.md` §§0, 1, 2a–2e on
      that build

Known non-goals for the PH demo (do not imply they ship):

- Native SwiftUI macOS app (`apps/macos`) — scaffold, not the public download
- MCP HTTP/relay — experimental-gated; pairing/approval only in stock builds
- Windows Authenticode (unless Path B completed)
- Cosign keyless provenance — documented in `docs/VERIFY_DOWNLOADS.md` but
  **not** attached to v2.0.3 assets (do not demo cosign verify until a release
  actually publishes `.cosign.sig` / `.cosign.pem`)

---

## 6. Cut / announce sequence (when gates pass)

1. `master` green (`make ci` or CI on the release commit).
2. If shipping current source: confirm manifests already at `2.0.4`, update
   site + README to **v2.0.4** in the same release PR, then tag `v2.0.4`.
3. Wait for `release.yml` to publish **both** platforms; confirm notes.
4. Re-run §2 (and §3 if claiming signed Windows) on the new assets.
5. Confirm site deploy + live download buttons.
6. Publish PH with the honest Win/Mac friction notes.

Do **not** edit `.github/workflows/release.yml` for launch day unless fixing a
proven one-line bug.

---

## Quick reference

| Doc | Use for |
|---|---|
| `RELEASING.md` | Tag flow, Apple/Azure secrets, post-release `spctl`/`stapler` |
| `docs/macos-signing-checklist.md` | Developer ID + notarization setup |
| `docs/azure-signing-cost.md` | Trusted Signing cost guardrails |
| `docs/windows-signing-checklist.md` | Authenticode options |
| `docs/VERIFY_DOWNLOADS.md` | SHA-256 / updater / (future) cosign |
| `docs/GHOST_2_DEMO.md` | 5-minute Action Plan demo |
| `docs/manual-qa-checklist.md` | Desktop QA gate |

**Last updated:** 2026-07-17  
**Status:** Checklist only — does not assert that v2.0.3 or unreleased 2.0.4
is Product Hunt ready until the boxes above are checked on real artifacts.
