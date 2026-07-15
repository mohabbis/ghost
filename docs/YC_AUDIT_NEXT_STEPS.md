# Ghost YC-Style Audit and Next Steps

## One-line thesis

Ghost should not try to be a general autonomous desktop agent yet. The wedge is: **a local-first recorder/replayer that makes one painful, repetitive desktop workflow reliable enough that a non-technical user trusts it weekly.**

## Current verdict

Ghost has a credible technical foundation, but it reads like a product that has accumulated many advanced-platform ideas before proving the narrow habit loop. The core app already exposes recording, replay, reliable replay, workflow save/load, AI generation, observer mode, diagnostics, visual checks, and data-source features in one screen. That breadth makes the product look ambitious, but it also hides the one thing a YC-stage product must prove first: users can record a real workflow, replay it successfully, and understand failures.

The repository already contains the right strategic framing in `docs/PRODUCT_ROADMAP.md`: reliability before AI, constrained AI, privacy as a feature, and avoiding enterprise/cloud distractions. This audit turns that into an execution plan.

## What is strong

1. **Clear underlying pain.** Desktop work remains full of repetitive cross-app workflows that do not have clean APIs.
2. **Local-first positioning is differentiated.** A desktop automation tool that watches input must compete on trust, not only capability.
3. **Native backend investment is directionally right.** macOS/Windows platform backends are the right place to create defensibility if replay reliability improves.
4. **The roadmap is more honest than the implementation docs.** The roadmap correctly says experimental AI/cloud/observer features should not be treated as production-ready until tested and documented.
5. **The app has a usable first loop.** Recording, timeline display, saving, loading, replaying, pausing, resuming, and permission gates are already represented in the frontend.

## Main risks

### 1. Scope is too wide for the stage

The current UI gives nearly equal weight to core recording/replay, AI draft generation, observer mode, visual checks, data sources, diagnostics, and reliable replay. That is too much surface area before the core loop is proven.

**Impact:** users will test the most magical-looking feature first, hit prototype behavior, and lose trust before discovering the reliable subset.

**Recommendation:** ship a narrow default experience: Record → Review → Test replay → Save. Move AI generation, observer mode, visual checks, and data-source tooling behind an “Experimental” section or feature flag.

### 2. Claims exceed implementation in several areas

`IMPLEMENTATION.md` presents cloud sync, enterprise audit logging, data-driven testing, visual regression, and observer mode as completed platform capabilities. In code, several cloud methods are in-memory or placeholder behavior, including token acceptance without server validation and sync returning workflow names rather than actually syncing.

**Impact:** overclaiming damages fundraising diligence and early-user trust.

**Recommendation:** split documentation into “available”, “prototype”, and “planned”. Remove “enterprise”, “cloud sync”, and “AI-powered platform” language from default marketing until those flows have production-grade backing.

### 3. Reliability is still the company

The strongest roadmap item says recording and replay must be “boringly reliable.” That is correct. The app should not be judged by how many automation primitives it has; it should be judged by task success rate across messy real apps.

**Impact:** if replay fails unpredictably, every other feature becomes decoration.

**Recommendation:** create a benchmark suite of 10 canonical workflows and track record/replay success as the north-star engineering metric.

### 4. Trust and safety need product-level guardrails, not just helper functions

The codebase contains security helpers and local auth/encryption concepts, but trust must be visible in the UX: recording indicators, sensitive-app warnings, blocklists, dry-run preview, emergency stop, and clear execution logs.

**Impact:** desktop observation feels creepy unless the product repeatedly proves that the user is in control.

**Recommendation:** make “what Ghost saw” and “what Ghost will do next” explicit before every replay.

### 5. The YC wedge is not “desktop AI agent”

“AI desktop agent” is too broad and will trigger skepticism. The YC wedge should be an urgent, repeated, specific workflow for a narrow user group.

**Recommended beachhead options:**

- Operations teams moving data between internal tools and spreadsheets.
- Recruiters/admins copying candidate/customer data across web apps.
- QA teams repeating desktop/web regression flows.
- Finance/admin users downloading, renaming, and filing recurring reports.

Pick one. Do not build for all of them at once.

## Recommended product focus

### ICP for the next 30 days

**Solo operators and ops-heavy small teams who copy data between web apps, spreadsheets, and desktop files weekly.**

Why this ICP:

- The pain is frequent and obvious.
- Workflows are repetitive but often not API-accessible.
- Users can evaluate value in one session.
- They tolerate local desktop software if it saves visible time.
- The workflow is less regulated than healthcare/banking/password-manager automation.

### Killer demo

“Record a 12-step weekly report workflow once. Move the browser window. Replay it. Ghost finds the right fields, pauses before submit, and produces a readable execution log.”

If this demo is not reliable, postpone everything else.

## North-star metrics

1. **Workflow replay success rate:** percentage of saved workflows that complete without manual intervention.
2. **Time-to-first-success:** minutes from install to first successfully replayed workflow.
3. **Weekly workflows replayed per active user.**
4. **Failure explainability:** percentage of failed replays with a specific actionable reason.
5. **User trust score:** qualitative: “Would you let Ghost run this while you watch?”

Avoid vanity metrics such as number of AI-generated workflows, number of feature buttons, or number of supported experimental modes.

## 30-day execution plan

### Week 1: Narrow the product

- Hide or label experimental buttons: AI generation, observer mode, visual checks, data sources, cloud/team language.
- Make the primary UI flow Record → Review → Test → Save.
- Add a “what can break this workflow” explanation after recording.
- Define 10 benchmark workflows across Chrome, Finder/File Explorer, spreadsheet/browser form, and email/reporting flows.

### Week 2: Make replay inspectable

- Add per-step replay status: pending, running, succeeded, failed, skipped.
- Add step-by-step replay.
- Add “retry from failed step”.
- Add dry-run preview for clicks: highlight target or show coordinates/element metadata before execution.

### Week 3: Improve target resilience

- Store and display multiple locator strategies per click: app, role, accessible name, window title, relative coordinates, absolute fallback.
- Prefer semantic lookup before absolute coordinates.
- Explain fallback decisions in the UI.
- Add benchmark logging so every replay produces a success/failure trace.

### Week 4: User discovery and paid pilot

- Interview 20 ops/admin/QA users.
- Watch 10 live installs and first recordings.
- Convert 3 workflows into documented templates.
- Charge for one narrowly defined pilot, even if manually supported.

## 90-day execution plan

1. **Reach 80%+ success on the 10-workflow benchmark.**
2. **Ship signed macOS and Windows builds with clean onboarding.**
3. **Launch three templates for the chosen ICP.**
4. **Add constrained AI only where it helps trust:** naming, summaries, failure explanations, and simplification suggestions.
5. **Publish honest docs:** local-first, permissions, limitations, supported apps, unsupported/sensitive surfaces.
6. **Collect workflow traces from opted-in testers and use them to prioritize reliability work.**

## What to stop doing for now

- Do not pitch enterprise audit logging.
- Do not pitch cloud sync as real unless it syncs with a real backend and conflict model.
- Do not lead with autonomous AI.
- Do not add more workflow primitives until replay debugging is excellent.
- Do not optimize the marketing site before the first successful repeated workflow cohort.

## Technical next steps

### P0

- Add a replay-debug data model with per-event status and failure cause.
- Persist execution traces locally.
- Add step-by-step replay and retry-from-step APIs.
- Create integration tests for serialization and replay state transitions.
- Fix Linux CI dependency assumptions or document required packages for `dbus-1` so tests run consistently.

### P1

- Introduce a multi-strategy `TargetLocator` model for UI events.
- Add dry-run replay mode.
- Add sensitive-app blocklist and confirmation prompts.
- Move cloud/observer/AI draft flows behind experimental settings.
- Update docs to label prototypes honestly.

### P2

- Real cloud sync only after local replay has strong retention.
- Team workspaces only after there is pull from paid pilots.
- Browser extension only if the chosen ICP repeatedly needs DOM-level reliability.
- Marketplace only after templates are already organically reused.

## Fundraising narrative

Use this framing:

> Ghost is Zapier for workflows that do not have APIs. It records repetitive desktop work locally, turns it into an inspectable workflow, and replays it with guardrails. We are starting with ops/admin workflows where people still copy data between tools by hand.

Avoid this framing:

> Ghost is an autonomous AI agent that controls your computer.

The second sounds bigger, but it invites fear, comparison to well-funded labs, and immediate reliability skepticism. The first sounds useful, specific, and sellable.

## Immediate decision

Pick one wedge and one benchmark workflow. The next milestone should be:

> A new user can install Ghost, record the benchmark workflow, replay it successfully after a window move, see exactly what happened, and save enough time to want to run it again next week.

Until that is true, every additional “advanced” feature should be treated as a distraction.
