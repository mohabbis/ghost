# Ghost: The Deterministic Trust Layer for Desktop Automation

## The One-Liner

Ghost lets non-technical teams automate sensitive desktop workflows without trusting black-box AI—by showing the plan, enforcing policy boundaries, and supporting one-click undo.

## The Problem

Every week, bookkeepers and office admins spend 2–3 hours organizing invoices, receipts, statements, and exports across folders and desktop apps. This work is repetitive, high-frequency, and audit-sensitive—but it is too risky for autonomous AI agents, too local for Zapier, and too small for enterprise RPA.

Existing tools don't fit:

- Macro recorders replay coordinates and break when windows move.
- Zapier/Make are great for cloud APIs, not local file workflows.
- UiPath costs $50k+ and requires implementation teams.
- AI agents are unpredictable and impossible to audit.

**The result**: Teams stay stuck doing boring work manually, and no tool has earned their trust for automation.

## Ghost's Answer

Ghost is not an autonomous agent. Ghost is a **deterministic trust layer** for desktop automation.

The core pipeline:

```
Intent → Plan → Policy Check → Human Approval → Execution → Audit → Undo
```

**Why it matters:**

1. **Deterministic** — Same input, same output. No hallucination.
2. **Reviewable** — Every step shown before execution.
3. **Policy-enforced** — Users define what Ghost may touch and what it may do.
4. **Auditable** — Full log of every operation.
5. **Recoverable** — One-click undo for file operations.

### The Demo (2 minutes)

1. User opens Ghost Organizer.
2. Selects `~/Downloads` as a trusted Zone.
3. Clicks "Scan."
4. Ghost proposes: "Move `invoice_acme.pdf` → `/Clients/Acme/Invoices/2026/`" with matched rule shown.
5. User reviews the before/after tree. Sees 43 files ready to move, 7 skipped.
6. Clicks "Approve."
7. Ghost executes with live progress, then shows: "43 moved, 31 renamed, 5 folders created, 7 skipped, 0 blocked. Undo available."
8. If anything looks wrong, one click: "Undo."

That's the product. Boring. Safe. Profitable.

## The Moat

1. **Deterministic compression engine** — Converts raw events into semantic, reviewable steps (proprietary algorithm).
2. **Policy engine** — Deny-by-default capability system. Competitors can't match without years of work.
3. **Local-first architecture** — Files never leave the machine. Compliance-friendly (GDPR, etc.).
4. **Undo and audit** — Most RPA tools don't have this. Users will pay for reversibility on sensitive workflows.

## Traction Targets (Before YC Application)

| Metric | Target | Proof |
|--------|--------|-------|
| Waitlist signups | 50–100 | Landing page + Product Hunt |
| Active beta users | 5–10 | Running Organizer workflows |
| Paid pilots | 3 | $99/year or $12/month seats |
| Testimonials | 3+ | Written case studies |
| Replay success rate | ≥99.5% | Canonical workflow benchmarks |
| Time saved per user | 2–3 hrs/week | User feedback |

## Business Model

Start simple. Premium on proof of value.

| Tier | Price | Users |
|------|-------|-------|
| Free | $0 | Test Organizer locally |
| Pro | $12/month | Bookkeepers, freelancers |
| Team | $29/user/month | Small teams with shared workflows |
| Enterprise | Custom | SSO, compliance exports, audit SIEM |

Focus: Free + Pro first, then Team when collaboration is real.

## Why Now

1. **RPA is broken for SMBs** — Too expensive, too complex, wrong trust model.
2. **AI agents are scary** — Users won't trust black boxes with sensitive files.
3. **Workflow automation is fragmented** — Desktop tools (Keyboard Maestro) don't have audit/undo. Cloud tools (Zapier) don't handle local files.
4. **Compliance is tightening** — Audit trails are becoming table stakes, not nice-to-have.
5. **Desktop automation has no trust model** — Ghost is the first to solve this.

## The Wedge: Ghost Organizer

Why file organization?

1. **Immediate ROI** — 2–3 hours/week saved is easy to measure.
2. **Compliance-driven** — Bookkeepers already track who accessed what files.
3. **Sensitive data** — Financial documents demand audit trails and undo.
4. **Repeatable** — Same workflow runs weekly → predictable usage.
5. **Clear UX** — Folder preview and file lists are familiar.

Once Organizer proves the model, expand to general workflows.

## What's Built

**Core systems (stable, tested, working):**

- ✅ **Ghost Organizer** — Scanner, classifier, planner (dry-run), policy check, executor, undo
- ✅ **Ghost Guard** — Deny-by-default policy engine
- ✅ **Zones and Capabilities** — Local boundary enforcement
- ✅ **Audit logs and undo journals** — Append-only, hash-chained
- ✅ **Event compression** — Raw events → semantic, reviewable steps
- ✅ **Replay execution** — Live progress, pause/resume, retry-from-failed-step
- ✅ **macOS and Windows builds** — Tauri 2, code ready for signing

**In progress:**

- ⏳ **Release signing** — Apple Developer ID notarization, Windows code signing
- ⏳ **Benchmark suite** — Canonical workflows with ≥99.5% success metrics

**Later (gated):**

- 🚫 **AI-assisted planning** — Suggestions only; execution deterministic
- 🚫 **Cloud sync** — Local-first always; sync is future, optional
- 🚫 **Unattended execution** — Not a current feature; user approval is the product

## Roadmap to YC Ready

### Phase 1: Scope Reduction (Weeks 1–2)
- Remove overpromising language and experimental UI
- Publish formal trust pipeline spec
- Define canonical workflows

### Phase 2: Reliability Foundation (Weeks 3–5)
- Benchmark suite with ≥99.5% target
- Publish metrics dashboard

### Phase 3: Production Hardening (Weeks 6–8)
- Code signing and notarization
- Security docs and threat model

### Phase 4: Organizer Polish (Weeks 9–10)
- First-run flow, dark mode, keyboard shortcuts

### Phase 5–8: Traction (Months 4–6)
- Private beta: 10 users
- 3 paid pilots
- Public beta
- YC application

## Why Ghost Wins

1. **Bottoms-up adoption** — SMBs adopt free, move to paid when they trust it.
2. **Network effects** — Organizer rules library + workflow sharing = virality.
3. **Compliance moat** — Audit trails and undo are table stakes in regulated industries.
4. **Margin expansion** — Desktop app + eventually cloud = SaaS-like unit economics.
5. **Team expansion** — Organizer solo → Ghost for all trusted automation.

## Team & Contact

Built by [founder name]. Seeking YC S26.

Code: Open source (MIT). Metrics: Canonical workflow benchmarks showing ≥99.5% success. Community: Contributing engineers welcome.

---

**Ghost is the trust layer desktop automation has always needed but never had.**

Record once. Review deterministically. Approve explicitly. Execute safely. Audit everything. Undo when needed.

That is the entire product. And users will pay for it.
