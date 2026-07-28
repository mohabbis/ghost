# Ghost Cloud — Phase 1 plan: Replay + Approval + Logs

Phase 0 shipped the running skeleton (workspace, data model, auth, app shell,
queue wiring). Phase 1 makes Ghost **do the job**: execute a workflow, pause for
human approval on sensitive steps, verify each outcome, and log the whole run.
This delivers MVP pillars **3, 4, 5**. Recording (pillars 1–2) stays for Phase 2.

Acceptance in one sentence: *create a workflow with a sensitive step, run it,
watch a live timeline fill with screenshots + verification, hit the approval
gate, approve, and see it finish — with a durable, hash-chained log.*

## Scope

In:
- Workflow + WorkflowVersion CRUD and a typed step editor (author steps by hand).
- A Playwright execution engine in the worker, driven by the `WorkflowStep`
  schema (semantic selectors, not pixels).
- Per-step screenshots, verification assertions, and `RunStep` records.
- Approval **halt/resume**: sensitive steps stop the run and wait for a human.
- Run list + Run detail with a live-updating timeline and approve/reject.
- Audit events (hash-chained) for run/step/gate/resume/finish.
- A seeded demo workflow that runs against a bundled fixture page.

Out (later phases): recording, connector/API steps execution, persistent
per-org browser context + stored credentials, SSE/WebSocket live updates
(Phase 1 uses polling), retries/backoff tuning, RBAC enforcement.

## Data model

No schema changes needed — Phase 0 already has `Run` (with `cursor`),
`RunStep`, `Approval`, and `AuditEvent`. Add only if a gap appears during build
(e.g. `Run.canceledById`); keep migrations additive.

## Execution engine (`apps/worker`)

New files under `apps/worker/src/`:

- `browser/driver.ts` — launches Chromium (reusing the preinstalled browser at
  `PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers`; no `playwright install`), owns a
  per-run context, exposes `runStep(page, step)`.
- `browser/selector.ts` — maps our `Selector` to a Playwright `Locator`,
  preferring semantic resolution: `role`+`name` → `getByRole`, `testId` →
  `getByTestId`, `text` → `getByText`, `css` → `locator(css)`. Semantic first,
  CSS last — mirrors the desktop resolution-chain invariant.
- `browser/verify.ts` — maps `verificationSchema` to Playwright assertions
  (`toHaveURL`, locator visibility, text present) → `{ passed, detail }`.
- `jobs/runWorkflow.ts` — the state machine below.
- `storage/artifacts.ts` — screenshot storage: S3 via `@aws-sdk/client-s3` when
  configured; dev fallback writes to `cloud/.artifacts/<runId>/<i>.png` served
  by a web API route. One interface, two backends.

### Run state machine (halt/resume is the core)

`Run.cursor` is the index of the next step to execute.

```
load steps = WorkflowVersion.steps
for i from run.cursor to end:
  if run.status == CANCELED: stop           # emergency-stop, checked each step
  step = steps[i]
  verdict = classifyStep(step)              # deterministic, from @ghost/core
  if verdict.sensitive and no APPROVED approval exists for i:
    create Approval(runId, i, reason=verdict.reason)
    run.status = AWAITING_APPROVAL; run.cursor = i
    audit("run.awaiting_approval", i); stop
  markStep(i, RUNNING); screenshot+verify; write RunStep; audit("step.*", i)
  if step failed: run.status = FAILED; stop
run.status = SUCCEEDED
```

Resume: when an `Approval` is resolved **APPROVED**, set `run.cursor = i + 1` and
re-enqueue `runWorkflow{ runId, fromStepIndex: i+1 }`. **REJECTED** →
`run.status = FAILED` (or CANCELED), no further steps. This keeps approval a
one-way gate the engine can't bypass — the trust guarantee.

Invariants carried from the desktop engine: interruptible (cancel checked each step),
deterministic execution of an approved plan, audit + (where applicable) undo.
Phase 1 mutations are browser-side and mostly non-destructive; the undo story
deepens with connector/file steps in later phases.

## Web app (`apps/web`)

- **Workflow CRUD** — `app/(app)/workflows/new` + `[id]` with a **typed step
  editor**: add/edit/reorder steps as a list, each row a form for its `type`
  (navigate url, click selector, fill value+sensitive, verify assertion,
  approval reason). Save creates a new immutable `WorkflowVersion`. Client-side
  validation reuses the Zod `workflowStep` schema from `@ghost/core`.
- **Run trigger** — a "Run" action enqueues `runWorkflow` and redirects to the
  run detail page.
- **Run detail** (`app/(app)/runs/[id]`) — server component + a client
  `RunTimeline` that **polls** `GET /api/runs/[id]` (~1.5s) for step statuses,
  screenshots (via signed/served URLs), and verification results. A pending
  `Approval` renders Approve / Reject calling
  `POST /api/runs/[id]/approvals/[stepIndex]`.
- **API routes** — `runs` (create/list/get), `approvals` (resolve),
  `artifacts/[...]` (serve dev screenshots). All auth- and org-scoped.

## Seed + fixtures

- A bundled fixture HTML (a tiny form with a "Submit order" button) served by the
  web app under a `/__fixtures/` route, so replay + the sensitive-gate path are
  testable with **no external network**.
- A `seed` script inserting a demo org + a demo workflow that navigates to the
  fixture, fills a field, and clicks "Submit order" (which the classifier gates).

## Tests

- Unit (`@ghost/core` already covers the classifier/schema/audit): add run
  state-machine unit tests (gate detection, cursor advance, resume, reject).
- Worker integration (vitest + Playwright): launch Chromium against the fixture,
  run the demo workflow, assert it halts at the gate, resolve the approval, and
  assert it completes with per-step `RunStep` rows + screenshots.
- Keep tests hermetic (fixture page, no public internet) so CI is deterministic.

## Dependencies / infra

- `apps/worker`: add `playwright` (+ `@aws-sdk/client-s3` for prod artifacts).
  Reuse the preinstalled Chromium; do not download browsers in CI.
- No new services beyond Phase 0's Postgres + Redis. S3 is optional in dev
  (disk fallback).

## Rollout

1. Execution engine + state machine + artifacts (worker), with the worker
   integration test — provable in isolation.
2. Workflow CRUD + step editor (web).
3. Run detail live timeline + approval resolve (web).
4. Seed + demo + docs; wire the "Run" button end to end.

Each is a small PR on top of the Phase 0 branch/merge. The `cloud/ci/cloud.yml`
workflow (once a maintainer installs it under `.github/workflows/`) gates all of
them; until then, run `pnpm typecheck && pnpm test && pnpm build` locally.

## Open decision still pending (Phase 2, flagged early)

Recording mechanism — server-side remote cloud browser (recommended, reuses the
run context + stored credentials) vs. Chrome extension. Phase 1 is built behind a
clean engine boundary so either recorder slots in without reworking execution.
