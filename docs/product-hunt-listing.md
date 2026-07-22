# Product Hunt listing — Ghost (ready to paste)

**Status:** Draft copy for a future PH launch. Docs only — does not assert the
build is launch-ready. Gate installs against `docs/ph-release-checklist.md`
before going live.

**Advertise download:** **v2.0.3** until a public GitHub Release for **v2.0.4**
exists with DMG + EXE assets. Source tree may already say `2.0.4`; that alone
is not a downloadable build (rule 10).

**Sources:** `docs/launch-demo-script.md`, `docs/yc-one-pager.md`, site hero in
`public/index.html`, positioning in `AGENTS.md` / `CLAUDE.md`.

---

## Name

```text
Ghost
```

Optional subtitle under the name (PH “tagline” field is separate):

```text
Client-data automation that never leaves your machine
```

---

## Tagline (≤60 characters)

Primary (40 chars) — paste into PH:

```text
Catch the wrong number at the keystroke.
```

Alternates if the primary is taken or feels too narrow:

| Tagline | Chars |
|---|---|
| Automate busywork. Catch the errors. | 36 |
| Replay data entry. Catch errors first. | 38 |
| Verify every keystroke. Halt on mismatch. | 41 |

Do **not** use agent / RPA / “autonomous” framing in the tagline.

---

## Description (PH product description)

Paste-ready:

```text
Ghost automates client-data workflows for the ops lead at a small wealth-management or accounting firm — without the data ever leaving the machine.

Cloud automation tools like Zapier and Make are banned outright by client-confidentiality policy. Ghost is the thing they're structurally disqualified from doing: it records a client-data transfer once, then replays it on your Mac or PC — and verifies every value against what you approved. A mismatch halts the run before the wrong number lands.

Trust pipeline, not a silent agent:
Record → Inspect → Approve → Replay → Verify → Audit → Undo

What you get today (technical preview v2.0.3):
• Explicit recording (visible when on — no camera, mic, or hidden observation)
• Reviewable compressed steps (not a black-box macro)
• Approve before anything mutates
• Per-step verification + halt on mismatch + on-device execution receipt
• Audited, reversible runs
• Ghost Organizer for safe local file filing (supporting capability)

No account required. Client data stays on your machine by default. $79/month per seat, flat.

Honest scope: today Ghost verifies per step (the approved value landed in the field). Full source-vs-destination reconciliation is roadmap — not claimed for this launch.
```

Shorter variant (~PH character comfort):

```text
Client-data automation for the ops lead at a small wealth-management or accounting firm — barred from cloud tools by confidentiality policy, so Ghost runs entirely on your Mac or PC. Record a data transfer once, approve the plan, then replay it. Ghost verifies each value against what you approved and halts on a mismatch. Audited and reversible. $79/seat, flat. Technical preview v2.0.3. No account required.

Not an autonomous agent, cloud RPA, or silent computer takeover. Approve-before-act is table stakes; catching the error is the wedge.
```

---

## Topics / tags

PH topic picks (choose up to what the form allows; prefer exact matches when available):

| Priority | Topic |
|---|---|
| 1 | Productivity |
| 2 | Developer Tools *(open-source desktop; optional)* |
| 3 | Artificial Intelligence *(only if forced — Ghost is not an AI agent; prefer skip)* |
| 4 | Mac |
| 5 | Windows |
| 6 | Finance |
| 7 | Open Source |
| 8 | Automation |

Suggested freeform / keyword tags:

```text
desktop automation, local-first, client data automation, wealth management,
accounting firm, data entry, verification, audit trail, undo, macOS, windows,
technical preview, trust pipeline
```

Avoid: “AI agent”, “RPA replacement”, “SOC 2”, “compliance certified”,
“ChatGPT app”, “fully reconciles your books”.

---

## Maker comment (first post)

Paste as the first maker comment on launch day. Edit the Windows SmartScreen
line only if you later ship a signed EXE.

```text
Hey Product Hunt 👋

I’m the maker of Ghost — local-first automation for the ops lead at a small wealth-management or accounting firm who's been told "we cannot put client data through a cloud tool."

The pitch in one line: you typed 12,900; the sheet says 12,090. Ghost catches it at the keystroke.

“Approve before it acts” is table stakes now. Ghost’s difference is what it checks before and as it runs: on each step it verifies the value you approved actually landed in the field. Observed ≠ expected → the run halts, and the exception is sealed into an on-device execution receipt. Every reversible change writes undo data first.

How a run works:
1. Record the transfer once (explicit, visible — never background observation)
2. Review compressed steps (readable values, not opaque coordinates)
3. Approve the plan
4. Replay on your Mac or PC
5. Verify per step → halt on mismatch
6. Audit + one-click undo

What’s different vs macros / RPA / “agents”:
• Macros replay blindly
• Classic RPA is brittle and rarely verifies the landing value
• Cloud agents often want your workbooks and credentials
Ghost stays on your machine, denies silent delete/overwrite/upload/send, and treats verification as the product — not an afterthought.

Honest non-claims for this launch:
• Not SOC 2 certified (and we’re not calling this a compliance product)
• No ChatGPT / OpenAI marketplace listing yet
• Verification today is per-step, not full source-vs-destination reconciliation (that’s next)
• Windows installer may be unsigned — SmartScreen “Unknown publisher” is expected; verify SHA-256 from the release notes, then More info → Run anyway
• Technical preview — Mac & Windows, no account required

Try it:
🌐 https://ghost.muharafiq.com/
⬇️ Download v2.0.3 (current public release): Mac DMG / Windows EXE via the site or GitHub Releases
📦 https://github.com/mohabbis/ghost

Happy to answer anything about the trust model, the mismatch-halt demo, or what’s roadmap vs shipped. Upvotes and tough questions both welcome.
```

---

## Gallery / screenshot shot list

Capture in this order (from `docs/launch-demo-script.md`). Crop tightly; match
the live app UI. Prefer the flagship verify story over Organizer.

| # | Shot | Must show | Gallery caption idea |
|---|---|---|---|
| 1 | Hero / product frame | Ghost app + spreadsheet context (optional: site hero for consistency) | You typed 12,900. The sheet says 12,090. |
| 2 | Recording | Explicit recording state / chrome | Record once — never in the background |
| 3 | Review timeline | Compressed steps + approved value on a type/enter step | Reviewable steps, not a black-box macro |
| 4 | Approve | Policy / consent before mutate | Nothing runs until you approve |
| 5 | Mismatch halt | `expected “12,900” · observed “12,090” → stop` | Halt before the wrong number lands |
| 6 | Execution receipt | Per-step verify / halt sealed | On-device receipt — Expected / Observed / Verified |
| 7 | Undo | Reversible run / history entry | Undo data written before each change |
| 8 | (Optional) Organizer | Scan → plan → approve → move | Supporting: safe local filing |

**Thumbnail / OG:** use `https://ghost.muharafiq.com/og.png` (1200×630) or a
still of shot 5 (mismatch halt) if PH wants a product UI crop.

---

## Demo GIF / video script beats

### 30-second social / PH media (preferred length)

| Beat | Time | On screen | VO / text |
|---|---|---|---|
| 1 | 0–5s | Hero headline or two sheets | “You typed 12,900. The sheet says 12,090.” |
| 2 | 5–13s | Record → review timeline | “Record the transfer once. Review readable steps.” |
| 3 | 13–25s | Approve → replay → **halt on 12,090** | “Approve, replay, verify — halt on mismatch.” |
| 4 | 25–30s | Receipt + download CTA | “v2.0.3 · local-first · no account. ghost.muharafiq.com” |

### 5-minute maker walkthrough (full)

Follow `docs/launch-demo-script.md` sections 0:00–5:00:

1. Hook + version line (v2.0.3 · local-first · no account)
2. Record (visible capture only)
3. Review timeline + approved value
4. Approve (do not skip)
5. Money shot: mismatch halt + receipt
6. Undo + close CTA

Film clean replay (optional) and mismatch halt (required); cut to the halt for
the short clip.

---

## Links

| Field | URL / value | Notes |
|---|---|---|
| Website | https://ghost.muharafiq.com/ | Canonical marketing site |
| GitHub | https://github.com/mohabbis/ghost | Open source |
| Download (advertise now) | **v2.0.3** via site `#download` or [GitHub Release v2.0.3](https://github.com/mohabbis/ghost/releases/tag/v2.0.3) | Latest *published* installer as of this draft |
| Mac DMG | https://github.com/mohabbis/ghost/releases/latest/download/Ghost.dmg | Resolves to latest *published* tag |
| Windows EXE | https://github.com/mohabbis/ghost/releases/latest/download/Ghost_Setup.exe | May be **unsigned** — disclose |
| v2.0.4 | Source may be at `2.0.4`; **do not** advertise as downloadable until that tag’s DMG/EXE exist | See `docs/ph-release-checklist.md` |

When pasting into PH “Get it” / download fields, prefer the website download
section so version copy stays centralized.

---

## What’s different (honest) vs macros / RPA / agents

| Alternative | Typical gap | Ghost |
|---|---|---|
| Macro recorders | Replay clicks/keys blindly | Compress → review → approve → **verify landing value** → halt on mismatch |
| Classic RPA | Brittle selectors; “ran” ≠ “correct” | Per-step Expected / Observed / Verified on an execution receipt |
| Cloud / “AI agents” | Often need cloud access + credentials; approval of a plan without proof it landed | Local-first; AI may propose, deterministic code executes only approved plans; no silent upload/send |
| “Approve before act” tools | Human gate only | Gate **plus** check that the approved value actually landed |

One-liner for comments:

> Approve-before-act is table stakes. Catching the error at the keystroke isn’t.

---

## Explicit non-claims (rule 10)

Do **not** say or imply any of the following on the PH page, maker comment,
gallery captions, or launch replies:

| Do not claim | Honest line instead |
|---|---|
| SOC 2 / ISO / “compliance certified” | Technical preview; not a compliance certification product |
| Listed on ChatGPT / OpenAI marketplace (or similar) | Not listed yet; local MCP pairing for Claude/Cursor is opt-in and still requires in-app approval |
| Full source-vs-destination reconciliation | **Per-step** verification ships; end-to-end reconciliation is **roadmap** |
| Windows is code-signed / SmartScreen-clean | Windows may be **unsigned**; disclose SmartScreen steps + checksums |
| Silent / always-on observation | Capture only during explicit recording or approved replay |
| Autonomous agent that takes over the computer | Human-approved routines with interruptible replay |
| Cloud-first storage of workflows | Local-first by default; integrations are opt-in and disclosed |
| Guard Desk / POS Bridge as certified compliance | Prototype desk workflows — labeled, not certified |
| “Download Ghost 2.0.4” before assets exist | Advertise **v2.0.3** until v2.0.4 is a real GitHub Release |

---

## Launch day checklist

Operational gate (detail in `docs/ph-release-checklist.md`). Do not publish
until the required boxes are checked or explicitly deferred with disclosure.

### Before publish

- [ ] PH download tag chosen: **reuse v2.0.3** or **cut v2.0.4** (only if demo needs unreleased bits)
- [ ] Site + README + `releases/latest` all show the **same** advertised tag
- [ ] DMG + EXE + `SHA256SUMS.txt` download and hash-check clean
- [ ] macOS: Gatekeeper / notarization verified on a clean Mac from the published DMG (not CI-claimed only)
- [ ] Windows: Path A (unsigned + SmartScreen FAQ in maker comment) **or** Path B (signed) — chosen explicitly
- [ ] Gallery shots 1–7 captured on **release bits** (not `cargo tauri dev`)
- [ ] 30s clip + optional 5min walkthrough uploaded; mismatch halt is the hero beat
- [ ] Honest-copy pass: no SOC2, no marketplace, no full reconciliation, no agent framing
- [ ] Maker accounts scheduled; first comment pasted within minutes of go-live

### At go-live

- [ ] Submit / publish PH listing with Name, Tagline, Description, Topics, Links above
- [ ] Pin maker comment (first post)
- [ ] Reply to early comments with trust-pipeline + non-claims as needed
- [ ] Monitor download links (200) and site deploy health
- [ ] Track “halt demo worked / install friction / Windows SmartScreen” feedback separately

### After day one

- [ ] Collect install friction (esp. Windows unsigned) for FAQ
- [ ] Do not bump advertised version to 2.0.4 until the tag’s installers exist
- [ ] File follow-ups from comments into GitHub issues without overpromising dates

---

## Quick paste block (fields only)

```text
Name:        Ghost
Tagline:     Catch the wrong number at the keystroke.
Website:     https://ghost.muharafiq.com/
GitHub:      https://github.com/mohabbis/ghost
Download:    v2.0.3 (until v2.0.4 is published) — site #download / GitHub Releases
Topics:      Productivity, Mac, Windows, Finance, Open Source, Automation
```

**Last updated:** 2026-07-17  
**Rule 10:** This file is launch copy scaffolding. It does not claim Product Hunt
readiness, signing completeness, or features beyond what v2.0.3 ships.
