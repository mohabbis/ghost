# Product Direction — Ghost Organizer First

This is the working product decision for Ghost while the market wedge is still
being clarified.

## Decision

Ghost should lead with **Ghost Organizer** before broad desktop replay.

The first user promise is:

```text
Pick a messy folder -> Preview the exact cleanup plan -> Approve -> Move/Rename -> Audit -> Undo
```

This direction is intentionally narrower than “automate my computer.” It gives
Ghost a concrete job, a short time-to-value loop, and a trust pipeline users can
understand after one run.

## Why Organizer first

Organizer is the best wedge because it is useful without asking users to trust
full computer control on day one:

- **Clear pain:** Downloads, client folders, month-end packets, exports,
  screenshots, PDFs, and duplicates pile up for almost everyone.
- **Low setup cost:** the user chooses one folder or Zone and can see value in
  minutes.
- **Visible trust:** every proposed move/rename is previewed before anything
  changes.
- **Local-first advantage:** files do not need to leave the machine for Ghost to
  be helpful.
- **Natural audit/undo:** the operation is deterministic, journaled, and
  reversible.
- **Expandable surface:** once Ghost is trusted with safe filing, routines and
  assistant connectors can propose Organizer plans without bypassing approval.

The wedge should not be “an autonomous desktop agent.” That claim is too broad
and weakens the safety posture that makes Ghost different.

## Target user for the first wedge

Start with people who repeatedly receive or create operational files and are
accountable for clean handoff:

- small business operators;
- finance/accounting/admin staff;
- consultants handling client deliverables;
- founders with messy Downloads/Desktop/client folders.

The first product should speak to one recurring moment:

> “I need this folder cleaned up safely, and I need proof of what changed.”

## Product loop

The default product loop is:

```text
Select folder -> Scan -> Propose plan -> Review -> Approve -> Move/Rename -> Audit -> Undo
```

Required UX properties:

1. The user always chooses the source folder.
2. The user always chooses the destination boundary or Zone.
3. Ghost previews every move and rename before execution.
4. Ghost flags conflicts, low-confidence items, and boundary denials.
5. Ghost never silently deletes files.
6. Ghost never silently overwrites files.
7. Ghost writes audit and undo data before mutation.
8. Undo is visible in the same place as history.

## What to build next

Prioritize work in this order:

1. **Make the Organizer path obvious in the app** — first-run guidance should
   start with “Create a Zone” and “Scan a folder,” not generic recording.
2. **Improve plan review quality** — group by destination, show conflict
   reasons, confidence, before/after paths, and counts.
3. **Strengthen policy and undo tests** — especially denied moves, conflicts,
   interrupted runs, and undo of partial runs.
4. **Add practical filing profiles** — Client filing, Bookkeeping inbox,
   Downloads cleanup; keep the planner deterministic.
5. **Add optional suggestions last** — AI can suggest categories or filenames,
   but deterministic Ghost code must validate, preview, and require approval
   before execution.
6. **Keep record/replay as a trust-core capability** — useful later, but not the
   homepage promise until reliability and verification are stronger.

## Positioning to use

Use language like:

> Ghost safely organizes messy local folders: it scans, proposes a cleanup plan,
> waits for your approval, moves/renames files, writes an audit log, and can undo
> the run.

Avoid language like:

- “Ghost controls your computer for you.”
- “Autonomous desktop agent.”
- “Always-on observer.”
- “AI runs your workflows.”
- “Set it and forget it.”

## Graduation criteria for broader automation

Record/replay and assistant-driven routines can become the headline only after
they have:

- reliable semantic targeting on macOS and Windows;
- per-step verification with clear mismatch handling;
- emergency stop and interruption tests;
- approval tokens bound to exact plan hashes;
- durable receipts and undo paths where practical;
- user-facing docs that accurately state limits.

Until then, broad automation remains a capability behind the Organizer-first
trust story, not the product thesis.

## Where this grows (without breaking the wedge)

Organizer-first is the beachhead, not the ceiling. For how Ghost expands into a
per-seat business along the trust/audit moat — verticals as playbook packs,
scheduled runs, and a team audit layer — without adopting the "workflow
automation" framing this doc forbids, see:

- `docs/automation-strategy.md` — the reconciliation of the wedge with the
  sellable platform underneath it;
- `docs/vertical-accounting-close.md` — the first vertical (month-end close),
  built on the existing Organizer pipeline;
- `docs/scheduled-runs-and-team-audit.md` — the recurring-revenue architecture,
  built on the existing Action Plan runtime and approval tokens.
