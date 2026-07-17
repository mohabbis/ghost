# Ghost launch demo script (Product Hunt / YC)

**Audience:** Product Hunt makers, YC partners, early finance/ops dogfooders  
**Build to advertise:** **v2.0.3** (Mac & Windows technical preview — local-first, no account required)  
**Length:** ~5 minutes on camera + optional 30s cut for social  
**Do not claim:** SOC 2, cloud-first sync, or shipped source-vs-destination reconciliation

Taglines that match the public site hero (#290):

| Use | Line |
|---|---|
| Hero / PH title | You typed 12,900. The sheet says 12,090. Ghost catches it at the keystroke. |
| Subtitle | Automate the busywork. Catch the errors. |
| Differentiator | Approve before it acts is table stakes. Catching the error isn’t. |
| Quote / closing | Automate the transfer, but catch the mistake — before it becomes someone’s problem in the close. |
| Trust strip | Record a task once · Verify every value · Halt on any mismatch · Audited & reversible |

Honest scope (say this once, early):

> Today Ghost verifies **per step** — the value you approved actually landed in the field. Full source-vs-destination reconciliation (every transferred figure vs its source of truth) is **roadmap**, not v2.0.3.

---

## Prep (5 minutes before recording)

1. Install **Ghost v2.0.3** from the public download page (Mac or Windows).
2. Grant **Accessibility** (and Input Monitoring on macOS if prompted). Recording/replay need them; Organizer does not.
3. Open two simple spreadsheets (or one sheet + a notes app) with a short cross-cell transfer:
   - Source cell shows `12,900`
   - Destination starts empty (or deliberately wrong for the mismatch beat)
4. Clear desktop clutter. Use a large UI zoom so the review timeline and receipt are readable on camera.
5. Optional: have a Downloads folder with one sample invoice ready if you want a 15s Organizer coda — keep the flagship story on **record → verify → halt**.

---

## 5-minute walkthrough

### 0:00–0:30 — Hook

On camera (or VO over the hero mock):

> Finance still moves numbers between spreadsheets by hand. One transposed digit hides until the close. Ghost records that transfer once, replays it on your machine, and **checks every value against what you approved**. A mismatch stops the run — before the wrong number lands.

Show: Ghost window + the two sheets. Say **v2.0.3 · local-first · no account required**.

### 0:30–1:15 — Record

1. Click **Record** (visible recording state — never background observation).
2. Perform the transfer once: copy/type `12,900` into the destination field, Tab/Enter as you normally would.
3. Stop recording.

Say out loud: Ghost only captures while you are explicitly recording. No camera, no mic, no hidden screen scrape.

**Screenshot:** recording indicator + the live event stream (or “recording” chrome).

### 1:15–2:15 — Review timeline

1. Open the compressed **review timeline** (event compression → semantic steps).
2. Point at a `TypeText` / enter-value step: show the **approved value** the step will write.
3. Note redaction defaults (typed text redacted in review unless retention is on; secure fields never retain secrets).

Say: raw clicks become reviewable steps — not a black-box macro.

**Screenshot:** review timeline with at least one typed-value step expanded.

### 2:15–2:45 — Approve

1. Run the routine policy / approval gate so every step (and each value it writes) is explicit.
2. Approve the plan. Do **not** skip this beat — trust is the product.

Say: AI may propose elsewhere; deterministic code executes **only** what you approved.

**Screenshot:** approval UI with policy decisions visible (Allow / Deny / review flags).

### 2:45–3:45 — Replay + mismatch halt (the money shot)

Two takes — film both; cut to the halt for the short cut:

**A. Clean replay (optional, ~20s)**  
Replay once so viewers see success: verify row shows expected = observed.

**B. Mismatch halt (required)**  
Before replaying, change the destination (or intervening UI) so the field will **not** match the approved value — e.g. leave `12,090` where `12,900` was approved, or edit the field mid-run if your demo path supports post-write verify.

Replay. When Ghost verifies the step and finds a mismatch:

- The run **halts**
- Nothing downstream is written as if the run succeeded
- The exception is sealed into the **execution receipt**

VO:

> Caught a mismatch — Ghost stopped before writing a wrong number. Nothing downstream changed.

**Screenshot:** halt / warning row — `expected “12,900” · observed “12,090” → stop` (matches site hero mock).

### 3:45–4:30 — Receipt

1. Open the **execution receipt** (Replay History / receipt view).
2. Walk Expected / Observed / Verified (or halt) per step.
3. Emphasize: this is an on-device audit of what happened — not a cloud log.

**Screenshot:** receipt with at least one verified step and the halted step.

### 4:30–5:00 — Undo + close

1. Undo the run (undo journal was written before reversible changes).
2. Show the destination restored / typed text reversed where undo applies.
3. Close with the quote tagline and download CTA for **v2.0.3**.

VO:

> Automate the transfer, but catch the mistake — before it becomes someone’s problem in the close. Download Ghost v2.0.3 for Mac or Windows. Local-first. No account required.

---

## Screenshot checklist (PH gallery / YC deck)

Capture in this order; crop tightly; prefer light UI if that’s what you ship on the site.

| # | Shot | Must show |
|---|---|---|
| 1 | Hero / product frame | Ghost app + spreadsheet context; optional site hero for consistency |
| 2 | Recording | Explicit recording state |
| 3 | Review timeline | Compressed steps + approved value on a type/enter step |
| 4 | Approve | Policy / consent before mutate |
| 5 | Mismatch halt | Expected vs observed; run stopped |
| 6 | Execution receipt | Per-step verify / halt sealed |
| 7 | Undo | Reversible run / history entry |
| 8 | (Optional) Organizer | Scan → plan → approve → move — supporting capability only |

---

## 30-second social cut

1. Hook line (hero headline) — 5s  
2. Record → review — 8s  
3. Approve → replay → **halt on 12,090** — 12s  
4. Receipt + “v2.0.3 · local-first” + download — 5s  

---

## Product Hunt copy stubs (honest)

**Tagline (≤60 chars):** Catch the wrong number at the keystroke.

**Description draft:**

Ghost is local-first desktop automation for finance teams. Record a repetitive data-entry transfer once, approve the plan, then replay it on your Mac or PC. Ghost verifies each value against what you approved and **halts on a mismatch** — so a transposed digit stops at the keystroke, not in the close. Audited and reversible. Technical preview **v2.0.3**. No account required.

**First comment topics:** trust pipeline (record → review → approve → replay → verify → undo); per-step verification vs roadmap reconciliation; Mac + Windows downloads; what you are *not* (autonomous agent, cloud RPA, SOC 2 certified).

---

## Claims guardrail (read before publish)

| Say | Do not say |
|---|---|
| Per-step verify: approved value landed in the field | “Reconciles source vs destination end-to-end” (roadmap) |
| Local-first by default; workflows stay on your machine | Cloud-first storage / “we sync your routines to our servers” |
| Technical preview v2.0.3 | Production-certified / SOC 2 / compliance product |
| Organizer is supporting safe filing | Organizer is the only story (flagship is verified transfer) |
| Prototype desk workflows stay labeled | Guard Desk / POS Bridge as certified compliance |

Related: `docs/GHOST_2_DEMO.md` (Action Plan runtime demo), site hero in `public/index.html`, positioning in `AGENTS.md` / `CLAUDE.md`.
