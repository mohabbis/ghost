# Ghost — Audiences

Ghost is a general-purpose, local-first tool, not a single-industry product. Its
value is a **trust pipeline** (preview → approve → execute → audit → undo) that
turns repetitive computer work into safe, reversible, permission-bounded
routines. That pipeline is audience-neutral; the *domain knowledge* that makes a
preview genuinely useful is layered on top as **profiles**.

This document defines who Ghost is for, what each audience needs, which features
serve them, and the concern each cares about most. Not everyone is a fit — for
some workflows Ghost adds little, and that is fine. We would rather be excellent
for a few audiences than mediocre for everyone.

## Primary audiences

### 1. Students

- **Job to be done:** file coursework (assignments, lectures, labs, exams,
  readings, projects, notes, syllabi) by course and academic term; rename messy
  downloads like `hw3_final_FINAL.pdf`.
- **Serving features:** the `Student` filing profile (`filing::academic`) — course
  code + term ("Fall 2026") + assignment-type detection; the Organizer for the
  actual safe move/rename.
- **Top concern:** free/cheap, works offline, nothing uploaded, nothing deleted.

### 2. Finance / operations admins

- **Job to be done:** file recurring financial reports (income statements,
  balance sheets, reconciliations, AR/AP aging, payroll, tax, budgets, bank
  statements, compliance reports) by type and reporting period; quantify the time
  and cost of doing it by hand.
- **Serving features:** the `Finance` filing profile (`filing::finance`) +
  period-based foldering; the savings estimator (`filing::savings`); the
  Organizer's audited, undoable execution.
- **Top concern:** regulated/PII data never leaves the machine, a human approves
  every change, and there is a durable audit trail with undo. Ghost must be
  assist-not-autonomy: it proposes, a person approves, the deterministic core
  executes.

### 3. Freelancers / creatives

- **Job to be done:** separate invoices, receipts, contracts, and deliverables by
  client and date.
- **Serving features:** Organizer categories (Invoices/Receipts/Statements),
  dated renaming, the client filing preset; period parsing.
- **Top concern:** clean client separation and zero accidental overwrite.

### 4. Legal / paralegal and researchers

- **Job to be done:** file documents by matter/case or project and date under a
  strict "never delete, never overwrite" rule.
- **Serving features:** Organizer with deny-by-default policy, the tamper-evident
  audit chain, and undo journal.
- **Top concern:** a defensible, chain-of-custody-style record of every change.

### 5. Developers, general and household users

- **Job to be done:** tidy Downloads — installers, media, archives, statements —
  quickly and reversibly.
- **Serving features:** the Organizer's deterministic classifier and safe
  executor.
- **Top concern:** simple, safe, reversible; no surprises.

## Who Ghost is *not* for (today)

- People who want a fully autonomous agent that acts without review — Ghost is
  deliberately human-in-the-loop.
- Cloud-first / multi-user collaboration workflows — Ghost is local-first.
- Anything requiring the app to read email, browser, or screen contents in the
  background — Ghost does not do hidden observation.

## Cross-audience guarantees

Every audience gets the same non-negotiables:

- local-first: no cloud dependency, no telemetry, no network calls in the stock
  build;
- no silent delete, no silent overwrite;
- preview before every mutation, approval required, audit written, undo
  available for reversible operations;
- AI (when enabled at all) only ever *suggests*; the deterministic core executes
  only what the user approved.

## How profiles are implemented

Profiles live in `src-tauri/src/filing/`:

- `period.rs` — shared reporting/academic period + date extraction from file
  names (pure, no IO);
- `finance.rs` — financial-report-type classification;
- `academic.rs` — coursework-type + course-code + term classification;
- `preview.rs` — the `Audience` enum and the read-only `preview_filing` planner;
- `savings.rs` — the time/cost savings estimator.

These are surfaced by the safe-read commands `preview_file_filing` and
`estimate_filing_savings` (`commands/filing.rs`), which touch **no filesystem**.
The actual move/rename is still performed only by the Organizer's audited,
undoable executor. See `docs/filing-profiles.md` for details.

Adding an audience is additive: a new profile module + an `Audience` variant +
tests, with no change to the trust pipeline.
