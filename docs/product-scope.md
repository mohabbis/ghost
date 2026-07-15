# Ghost Product Scope

## What Ghost Is

Ghost is a **local-first, deterministic, auditable desktop automation engine** for sensitive workflows.

**Core identity**: The trust layer. Every operation passes through explicit approval, policy checks, audit logs, and undo.

**Positioning**:
- NOT an autonomous AI agent.
- NOT a cloud-first service.
- NOT a macro recorder that replays blind coordinates.
- NOT Zapier/Make (which handle cloud APIs well, not local desktop workflows).
- NOT enterprise RPA (which is expensive and overbuilt for SMBs).

Ghost is for **deterministic, auditable, user-approved desktop automation**.

---

## What Ghost Does (MVP)

### Ghost Organizer (The Killer Workflow)

**Scope**: Safe file organization with preview, policy enforcement, audit, and undo.

**Flow**:
1. Select a folder (Zone).
2. Scan files.
3. Classify using deterministic rules.
4. Propose moves and renames.
5. Review plan with before/after preview.
6. Approve within policy boundaries.
7. Execute with live progress.
8. Audit every operation.
9. Undo if needed.

**File operations supported**:
- ✅ Move files within Zone.
- ✅ Rename files.
- ✅ Create folders (as needed).
- ✅ Skip ambiguous files (low confidence).
- ❌ Delete files (blocked by default).
- ❌ Overwrite without approval (blocked by default).

**Classification rules** (deterministic, not ML):
- Filename contains invoice → Invoice
- Filename contains receipt → Receipt
- Filename contains statement → Statement
- Client code + document pattern → Client filing
- Extension `.csv`, `.xlsx` → Spreadsheet export
- Unknown pattern → Skip

**Preset configurations**:
- Invoice filing (client folder structure).
- Receipt organization (by vendor/date).
- Statement sorting (by bank/month).
- Desktop cleanup (by file type).
- Project archival (by modification date).

**Zones** (boundaries):
- User selects which folders Ghost may access.
- Deny-by-default: Ghost cannot touch anything outside the Zone.
- Examples: `~/Downloads`, `~/Desktop`, `~/Documents/Clients`.

**Capabilities** (what Ghost may do in a Zone):
- `Read` — Scan files, list folder structure.
- `Create` — Create folders as needed.
- `Move` — Move files within Zone.
- `Rename` — Rename files.
- `Delete` — Blocked by default (future: opt-in with audit).

**Approval workflow**:
- User sees plan before any changes.
- Can approve all, deselect specific actions, edit destinations, save as preset.
- Cannot proceed without approval.

**Audit and undo**:
- Append-only audit log (every operation recorded).
- Undo journal (before-state snapshots, reversibility metadata).
- User can export audit as CSV/JSON.
- One-click undo (reverses entire run).

**Performance targets**:
- Scan 1,000 files: < 5 seconds.
- Plan generation: < 2 seconds.
- Policy evaluation: < 50ms per file.
- Execution: 100 files/sec (with live progress).

---

## What Ghost Does NOT (Out of Scope for MVP)

### Recorded Workflows / Routines

❌ **Not in MVP**. Future Phase 2.

Why later:
- Requires robust replay (semantic targeting + fallback strategies).
- Cross-app replay is complex and fragile.
- Organizer is the higher-ROI wedge first.

When added (Phase 2):
- Users record clicks, keystrokes, scrolls.
- Compressed into semantic steps.
- Reviewed and approved before replay.
- Same trust pipeline as Organizer.

### Cloud Sync

❌ **Not in MVP**. Not planned for Phase 1.

Why not:
- Local-first is non-negotiable. Sync is optional future.
- Adds attack surface (network, account, cloud data).
- MVP success is local-only.

If added (later):
- Optional, opt-in, encrypted, user-controlled.
- Never a requirement to use Ghost.

### AI/ML Features

❌ **Not in MVP**. Not in Phase 1 or 2.

Why not:
- Deterministic classification is better for trust.
- ML adds unpredictability and audit risk.
- Pattern matching > neural networks for this use case.

If added (Phase 3+, gated):
- Suggestion-only (no execution without approval).
- Deterministic code executes approved plans.
- User can always override suggestions.

### Autonomous Execution

❌ **Never by default.**

Why not:
- User approval is the product.
- Scheduled/background execution without review is a trust violation.
- SMBs (the target wedge) want control, not automation "magic."

If added (very future, heavily gated):
- Require explicit policy authoring and review.
- Strict audit requirements.
- Always interruptible.

### Visual Regression Detection

❌ **Not in MVP**.

Why not:
- Useful for recording workflows, not file organization.
- Out of scope for Organizer.
- Deferred to Phase 2+ (if recorded workflows materialize).

### Unattended/Scheduled Replay

❌ **Not in MVP**.

Why not:
- Ghost is synchronous, interactive.
- User approval model requires presence.
- Scheduled execution is a future tier.

### Multi-Device Sync

❌ **Not planned**.

Why not:
- Desktop-only for now.
- Mobile/web are out of scope.
- Focus on macOS + Windows excellence first.

### SSO / Managed Policies

❌ **Not in MVP**.

Why not:
- Enterprise features. MVP is individual + small teams.
- Local auth only initially.

When added (Phase 1 after MVP is proven):
- Team tier with shared workflows and centralized audit.
- Eventually: SSO, managed policies, admin dashboard.

---

## Core Features (Stable, Tested, Working)

| Feature | Status | Used In |
|---------|--------|---------|
| Zone creation and management | ✅ Stable | Organizer |
| Capability system (Read/Create/Move/Rename) | ✅ Stable | Organizer |
| Ghost Guard policy engine (deny-by-default) | ✅ Stable | Organizer |
| File scanning and classification | ✅ Stable | Organizer |
| Plan generation (dry-run, read-only) | ✅ Stable | Organizer |
| Execution with live progress | ✅ Stable | Organizer |
| Audit logging (append-only, hash-chained) | ✅ Stable | Organizer |
| Undo journals and recovery | ✅ Stable | Organizer |
| Workflow storage (local JSON) | ✅ Stable | Organizer presets |
| Tauri 2 desktop shell (macOS + Windows) | ✅ Stable | All |
| Local authentication (optional password) | ✅ Stable | All |
| Performance monitoring and telemetry | ✅ Stable | All |

---

## Features In Progress

| Feature | Target | Notes |
|---------|--------|-------|
| Apple code signing and notarization | Phase 3 | Security hardening |
| Windows code signing | Phase 3 | Security hardening |
| Benchmark suite (≥99.5% success) | Phase 2 | Reliability proof |
| Better error messages | Phase 4 | UX polish |
| First-run onboarding | Phase 4 | UX polish |
| Dark mode | Phase 4 | UX polish |
| Keyboard shortcuts | Phase 4 | UX polish |

---

## Features Explicitly Out of Scope (Not Doing)

| Feature | Reason |
|---------|--------|
| Autonomous execution (no approval) | User approval is the moat. |
| Cloud data collection | Local-first by default. |
| Unattended/scheduled workflows | Synchronous, interactive by design. |
| Mobile or web interface | Desktop-first. |
| Visual regression checks | Not needed for file organization. |
| AI suggestions (enabled by default) | Determinism and trust > flexibility. |
| Macro recording (coordinate-based replay) | We're doing semantic compression instead. |
| System-wide automation without Zones | Permission boundaries are non-negotiable. |
| Silent overwrites or deletes | Preview and approval always required. |
| Storing secrets or sensitive input | Never retained, even with audit. |

---

## Why This Scope Makes Sense

### Wedge Product Strategy

Ghost Organizer is boring on purpose. Why?

1. **High frequency** — Bookkeepers do this every week.
2. **Sensitive** — Financial files demand audit and undo.
3. **Clear UX** — Folder previews are familiar.
4. **Measurable ROI** — 2–3 hours/week saved is easy to quantify.
5. **Compliance-driven** — Auditors expect audit trails anyway.

Once Organizer proves the wedge, expand:
- Phase 2: Recorded workflows (semantic compression → universal automation).
- Phase 3: Intelligence (suggestion-only ML, gated).
- Phase 4+: Cloud, team sync, enterprise features.

### Why NOT Broad Desktop Automation First

❌ Trying to do "record anything, replay anything" first is a trap:

- Cross-app replay is fragile (coordinate dependency, window moves, app updates).
- AI is tempting but breaks trust (unpredictable, unauditable).
- Feature sprawl dilutes focus (looks impressive, ships broken).

✅ Focus on one killer workflow ensures:

- Reliability can be measured and proven (≥99.5% success).
- Trust is earned through repetition and consistency.
- UX can be polished without scope creep.
- Unit economics can be validated with real users.

---

## Principles

1. **User approval is the product** — Not convenience. Not magic. Approval.
2. **Deny by default** — Everything is blocked unless explicitly allowed.
3. **Deterministic > intelligent** — Predictability > flexibility.
4. **Audit everything** — Every operation is logged with full context.
5. **Undo first** — Write undo data before executing.
6. **Local-first always** — Files never leave the machine by default.
7. **One workflow at a time** — Ship one thing that works instead of five things that don't.

---

## Success Metrics

| Metric | Target | Why |
|--------|--------|-----|
| Organizer success rate | ≥99.5% | Reliability proof |
| Undo success (for supported operations) | 100% | Safety guarantee |
| Time to approve/execute | < 30 seconds for 100 files | UX responsiveness |
| Audit log export | < 1 second | User experience |
| Time saved per user per week | 2–3 hours | Business case |
| Active monthly users | 10+ in beta | Traction |
| Paid pilots | 3+ firms | Willingness to pay |
| NPS (net promoter score) | > 50 | Product/market fit signal |

---

## Timeline

**Phase 0** (Week 0): Decision and cleanup
- Repositioning (trust layer, not AI agent)
- Scope reduction (Organizer first)

**Phase 1** (Weeks 1–2): Clear communication
- Publish specs (trust pipeline, product scope)
- Define canonical workflows

**Phase 2** (Weeks 3–5): Reliability foundation
- Benchmark suite with ≥99.5% target
- Comprehensive tests

**Phase 3** (Weeks 6–8): Production hardening
- Code signing and notarization
- Security docs

**Phase 4** (Weeks 9–10): UX polish
- First-run flow, dark mode, shortcuts

**Phase 5–8** (Months 4–6): Traction
- Private beta (10 users)
- 3 paid pilots
- Public beta
- YC application
