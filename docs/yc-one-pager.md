# Ghost — YC one-pager

**Product:** Ghost v2.0.3 (Mac & Windows technical preview)  
**Line:** You typed 12,900. The sheet says 12,090. Ghost catches it at the keystroke.

## Problem

Finance and ops teams still move numbers between spreadsheets and systems by hand. A mistyped or mis-pasted figure is expensive — and it usually surfaces in the close, after it has already flowed downstream. Macro recorders replay blindly. Cloud RPA and “AI agents” often ask for approval of a plan but do not verify that the approved value actually landed.

## Solution

Ghost is local-first desktop automation: **Record → Review → Approve → Replay → Verify → Undo**.

- Record an explicit data-entry routine once (visible capture only — no camera, mic, or hidden observation).
- Compress raw input into a reviewable timeline; approve each step and the value it will write.
- Replay with deterministic code on the user’s Mac or PC.
- **Verify per step** against the approved value; **halt on mismatch** and seal an execution receipt.
- Write undo data before reversible changes.

Differentiator: “Approve before it acts” is table stakes. Ghost’s wedge is **catching the error at the keystroke**.

## Wedge

**Vertical:** financial-services / finance-ops teams.  
**Flagship workflow:** cross-sheet / cross-system data entry with verification.  
**Supporting capability:** Ghost Organizer (scan → plan → approve → move/rename → audit → undo) — keeps outputs tidy; not the headline.

Not positioning as: generic autonomous agent, chatbot, RPA clone, or silent computer takeover.

## Why now

Desktop work still lacks clean APIs for the messy middle of the close. Teams already trust local tools more than uploading workbooks and credentials to a cloud agent. Verification-as-product is newly legible: every serious automation waits for a human; Ghost checks its own work before and as it acts. Mac + Windows technical preview (**v2.0.3**) is downloadable without an account.

## Trust model (local-first)

```text
Intent → Plan → Policy check → User approval → Execution → Audit log → Undo path
```

- Deny-by-default policy; no silent delete/overwrite/upload/send.
- AI may propose; deterministic code executes only approved plans.
- Workflow and organizer data stay on-device (encrypted at rest when a vault password is set).
- Account sign-in and stack integrations (e.g. Power BI export) are opt-in and disclosed — never a way around the trust pipeline.
- Risk classes on every command surface (`safe-read` … `experimental`).

## Shipped vs roadmap

| Shipped in v2.0.3 (honest) | Roadmap / not claimed |
|---|---|
| Record, compress, review timeline, approve, replay | Full **source-vs-destination reconciliation** |
| **Per-step** verify + mismatch halt + execution receipt | SOC 2 / compliance certification |
| Audit + undo; Organizer plan/execute/undo | Cloud-first routine storage |
| Local-first default; no account required to try | Broad autonomous multi-app “agent” |
| Technical preview on Mac & Windows | Guard Desk / POS Bridge as certified compliance |

## Ask

Dogfood with finance teams who live in spreadsheets weekly; measure whether a verified transfer (especially a halted mismatch) builds trust in one session. Demo script: `docs/launch-demo-script.md`.
