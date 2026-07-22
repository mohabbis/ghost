# Ghost — Audiences

Ghost's current product wedge is **Ghost Organizer**. The first buyer is anyone
accountable for recurring operational files who needs a safe cleanup plan, not a
silent autonomous agent.

**Primary buyer:** small-business operators and finance/accounting/admin staff
who repeatedly handle client folders, Downloads, month-end exports, PDFs,
screenshots, and handoff packets.

**Price hypothesis:** $79/month per seat. Keep it flat while validating
willingness to pay; do not add tiers or “contact sales” until there are
reference users and a support motion.

**Flagship workflow:** *messy local folder → reviewed cleanup plan → approved
move/rename → audit trail → undo.* Ghost scans only the selected folder/Zone,
previews every action, flags conflicts, never silently deletes or overwrites,
and writes undo data before mutation.

Its value is the **trust pipeline** — select → scan → propose → review →
approve → execute → audit → undo — applied to local files where a bad move or
overwrite is costly and users need confidence before trusting broader automation.

## Who Ghost is for

### The operator with recurring messy folders

- **Job to be done:** clean up Downloads, client folders, export folders,
  month-end packets, screenshots, PDFs, and handoff directories without losing
  track of what moved.
- **Serving features:** Organizer Zones; scan/preview; deterministic filing
  profiles; conflict and low-confidence detection; audited execution; one-click
  undo.
- **Top concern:** Ghost must not silently delete, overwrite, upload, or move
  files outside the approved boundary. The user needs a readable plan and proof
  of what changed.

### The ops lead at a small wealth-management or accounting firm

- **Job to be done:** keep client documents, statements, exports, and
  workpapers filed into predictable folders before or after month-end data-entry
  routines.
- **Serving features:** the `Finance` filing profile + period foldering,
  Organizer audit chain, undo journal, and later record/replay for verified
  re-keying once the broader automation surface is ready.
- **Top concern:** client data stays local; a human approves every change;
  mistakes can be audited and reversed.

## Cross-audience guarantees

Every user gets the same non-negotiables:

- local-first: no cloud dependency, no telemetry, no network calls in the stock
  build; regulated data never leaves the machine;
- assist-not-autonomy: AI may *suggest*; the deterministic core executes only what
  the user approved;
- review before commit: each file move/rename is previewed, conflicts are
  flagged, and execution is denied unless policy passes;
- no silent delete, no silent overwrite; audit written, undo available for
  reversible operations.

## What Ghost is *not* for (today)

- A fully autonomous agent that acts without review — Ghost is human-in-the-loop.
- Cloud-first / multi-user collaboration — Ghost is local-first.
- Background observation of email, browser, or screen — Ghost does not do it.
- Broad desktop control as the first promise. Record/replay remains a trust-core
  capability, but Organizer is the first product wedge.
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
