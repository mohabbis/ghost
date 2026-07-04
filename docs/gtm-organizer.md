# Go-to-market: Ghost Organizer

Operational GTM notes for the wedge. Keep this a checklist, not a vision essay
(per `AGENTS.md`). Update it when positioning or the beachhead changes.

## Beachhead persona

**Bookkeepers, practice admins, and finance-adjacent solo professionals** who
file client documents every week — invoices, receipts, and statements pulled
from portals into a mess of Downloads folders.

Why this persona:

- The pain is **recurring and concrete** (weekly filing), not aspirational.
- It is **audit-sensitive**: these users already think in terms of "what moved,
  and can I show it." Ghost's rule-attributed, exportable audit log is a feature
  they understand instantly, not a developer nicety.
- Ghost already ships the matching capability: invoice/receipt/statement
  classification, dated renaming, the client-filing preset, and the guided-setup
  wizard (`organizerRunWizard` in `src/main.js`).

Do **not** broaden to "everyone with a messy Downloads folder" in messaging yet.
The general case is the demo; the bookkeeper is the buyer.

## Positioning

One line:

> Ghost files your client documents on your own machine — you set automate,
> ask-first, or never per folder, and every change names the rule that fired.

Against **cloud agentic back-office tools** (e.g. Rational-style "agentic
employees"): they sign into your accounting systems in their cloud and hold your
logins. Ghost is the local-first inverse — nothing uploaded, no vendor holds
credentials, the same trust mechanics (per-rule trust levels, ask-first
escalation, rule-attributed exportable audit) but on-device. Lead with that
contrast; it is the differentiation, not a gap.

Against **RPA / macro tools**: those record brittle scripts and run them blindly.
Ghost previews every mutation, denies silent delete/overwrite, and is reversible.
"Preview → approve → audit → undo," not "record → pray."

## Trust mechanics to lead with (all shipping)

- **Per-folder trust levels**: automate / ask-first / never.
- **Rule attribution**: every audited change names the boundary that authorized
  it and whether it was automated or you-approved.
- **Exportable audit**: CSV or JSON, per run (`organizer_export_audit`).
- **Never overwrites, never deletes; one-click undo.**
- **Guided setup**: a short interview becomes a ready, reviewable boundary.

## Outcome-math template (for future case studies)

Fill in from a real user; do not fabricate numbers.

> A [role] filing [N] client documents/week cut filing from [X] hours to
> [Y] minutes — every move audited to the rule that fired, and reversible.
> Ghost ran entirely on their laptop; no client data left the machine.

## Time-to-value claim (gated)

The site must not print a "first safe cleanup in N minutes" number until Phase 4
instrumentation (`organizer_time_to_value`, local-only) yields real medians.
Measure first, then publish (per `CLAUDE.md`: no promises the app can't support).

## Channels

- Bookkeeper / accounting-tech communities (e.g. r/Bookkeeping, r/smallbusiness,
  practice-management forums, accounting-software subreddits).
- Accounting-tech newsletters and creators.
- Product Hunt launch, framed around the trust model + the bookkeeper niche.
- The marketing site's existing `#client-filing` section and client-filing demo
  tab are the on-site conversion surface; keep them in sync with shipped product.

## Guardrails for marketing copy

- Never frame Ghost as an autonomous agent, chatbot, RPA clone, or macro
  recorder (per `CLAUDE.md` product identity).
- Only claim capabilities the app actually supports today. When a feature is
  roadmap, put it under "Next"/"Later," not "Shipping now."
