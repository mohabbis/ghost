# Ghost — Audiences

Ghost has one buyer and one flagship workflow. Everything on the public surface
reinforces the same story; capability that serves a different persona stays off
the front page until the wedge has real, referenceable users.

**Primary buyer:** the ops lead at a 10–50 person wealth-management or accounting
firm who has been explicitly told "we cannot put client data through a cloud
tool." Not "SMBs." Not "professionals." That specific person, with that specific
constraint.

**Price:** $79/month per seat. Flat — no tiers, no "contact sales."

**Flagship workflow:** *client data stuck in email/PDFs → automated, verified
transfer into the CRM or portfolio system → human reviews the exceptions.* Ghost
records the re-keying/copy-across you do every period, replays it, verifies every
value against what you approved, and halts on any mismatch so a bad figure never
flows downstream — entirely on the machine, because cloud automation tools
(Zapier, Make, etc.) are structurally disqualified by client-confidentiality
policy.

Its value is the **trust pipeline** — record → review → approve → replay →
verify → undo — applied to the one class of work where a single wrong number is
expensive, a clean audit trail is not optional, and the data cannot leave the
building.

## Who Ghost is for

### The ops lead at a small wealth-management or accounting firm

- **Job to be done:** moving client data between email, PDFs, and the firm's
  CRM/portfolio system — month-end re-keying, bank/statement exports and
  workbooks, recurring reconciliations and report prep.
- **Serving features:** Record → replay with per-step verification (the value you
  approved actually landed); the tamper-evident audit chain and undo journal; the
  `Finance` filing profile + period foldering for the documents produced.
- **Top concern:** client data is barred from cloud tools by firm policy, so it
  never leaves the machine; a human approves every change; a wrong value is
  caught before it ships; there is a durable, reversible audit trail.

## Cross-audience guarantees

Every user gets the same non-negotiables:

- local-first: no cloud dependency, no telemetry, no network calls in the stock
  build; regulated data never leaves the machine;
- assist-not-autonomy: AI may *suggest*; the deterministic core executes only what
  the user approved;
- verify before commit: each step confirms the approved value landed; a mismatch
  halts the run;
- no silent delete, no silent overwrite; audit written, undo available for
  reversible operations.

## What Ghost is *not* for (today)

- A fully autonomous agent that acts without review — Ghost is human-in-the-loop.
- Cloud-first / multi-user collaboration — Ghost is local-first.
- Background observation of email, browser, or screen — Ghost does not do it.
- General-purpose file cleanup for every persona (students, household, legal,
  freelancers, developers). Ghost *can* file those documents safely — the
  Organizer trust pipeline is audience-neutral — but that is a supporting
  capability, not who we position for or build features around.
- "Workflow automation" or an "operating system" — one product, one workflow, not
  a category-level platform.
- A multi-provider LLM routing platform — which model runs underneath isn't the
  pitch.
- Enterprise motion — SSO, compliance questionnaires, "contact sales" — parked
  ~18 months out, not now.

## Scope discipline

The codebase carries more than this document lists (Routines beyond finance,
on-device OCR/ID parsing, a semantic-memory graph, an MCP approval surface,
experimental AI providers, optional identity sign-in and stack integrations).
Those stay off the default surface and out of the primary pitch. A new audience or
feature earns a place on the front page only when pull from real users in the
vertical above justifies it — not engineering momentum.

## How profiles are implemented

Filing profiles live in `src-tauri/src/filing/` (`period.rs`, `finance.rs`,
`academic.rs`, `engineering.rs`, `preview.rs`, `savings.rs`) and are surfaced by
the read-only commands `preview_file_filing` / `estimate_filing_savings`
(`commands/filing.rs`), which touch **no filesystem**. The actual move/rename is
performed only by the Organizer's audited, undoable executor. See
`docs/filing-profiles.md`. The `Finance` profile serves the primary audience
above; the others remain in the code but are not front-page positioning.
