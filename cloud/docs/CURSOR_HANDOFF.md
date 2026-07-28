# Ghost Cloud — handoff for Cursor

You're picking up **Ghost Cloud**, the cloud SaaS rebuild of Ghost: an AI
operator that learns a business workflow once, then executes it across existing
software with **human approval on sensitive actions, verification, and a full
audit log**. All new work lives under `cloud/` (a self-contained pnpm +
Turborepo workspace). The legacy Rust/Tauri desktop app at the repo root is
untouched and out of scope.

Read `cloud/README.md` and `cloud/docs/PHASE_1_PLAN.md` first — this file is the
"where we are / what to do next" layer on top of them.

## The one thing to do first: verify the end-to-end loop with a real DB

Everything below is written and green on `typecheck` + `build` + unit tests, but
the **DB-backed run flow has not been executed end-to-end** in the environment it
was built in (no Docker/Postgres there). Your first job is to run it and fix
anything that surfaces.

```bash
cd cloud
cp .env.example .env
#  In .env set at minimum:
#    AUTH_SECRET   -> `npx auth secret` or `openssl rand -base64 32`
#    GHOST_ARTIFACT_DIR -> an ABSOLUTE path both web+worker share, e.g. /abs/path/to/cloud/.artifacts
#    APP_URL       -> http://localhost:3000   (worker reaches the demo fixture here)
pnpm install
docker compose up -d                 # Postgres :5432, Redis :6379
pnpm db:migrate                       # applies packages/core/prisma/migrations
# two terminals (or use `pnpm dev` which runs both via turbo):
pnpm --filter @ghost/web dev          # http://localhost:3000
pnpm --filter @ghost/worker dev
```

Then in the browser:

1. Sign in (dev provider accepts any email; an Organization is auto-created).
2. **Workflows → Create demo workflow → Run.** You land on the run page.
3. Watch the timeline: navigate → fill run and screenshot; it **halts at
   "Submit order" (AWAITING_APPROVAL)** because the classifier flags it.
4. Click **Approve** → the run resumes, submits, verifies "Order submitted", and
   finishes **SUCCEEDED**. **Reject** instead → run FAILS without submitting.

If screenshots don't render, it's almost always `GHOST_ARTIFACT_DIR` not being the
**same absolute path** for both processes (see `.env.example`). If the worker
can't launch Chromium, run `pnpm --filter @ghost/worker exec playwright install
chromium` (locally you won't have `/opt/pw-browsers`; the driver falls back to
Playwright's default cache).

## Architecture (what's where)

```
cloud/
  packages/core/         Prisma schema/client, Zod workflow-step schema,
                         deterministic sensitive-action classifier, audit hash chain
  apps/web/              Next.js 15 (App Router) + Tailwind v4 + Auth.js v5
  apps/worker/           BullMQ consumer + Playwright execution engine
```

**Trust model (do not regress):** AI may propose, deterministic code executes
approved plans. The approval gate is enforced in a **pure state machine**
(`apps/worker/src/runtime/state-machine.ts`) using
`@ghost/core`'s `classifyStep` — no AI in that decision. Every run/step/gate/
finish appends to a per-org **hash-chained** `AuditEvent`
(`apps/worker/src/jobs/runWorkflow.ts` → `appendAudit`). Keep it that way.

**Execution flow (built, Phase 1.1 + 1.2):**
- Trigger: `POST /api/runs {workflowId}` creates a `Run` (QUEUED) and enqueues
  `runWorkflow` (`apps/web/src/app/api/runs/route.ts`).
- Worker: `runWorkflowJob` loads the run, walks steps from `run.cursor` via
  `planNextAction`. Executes each via the Playwright driver
  (`apps/worker/src/browser/driver.ts`; semantic selectors in `selector.ts`,
  assertions in `verify.ts`), screenshots to the artifact store
  (`storage/artifacts.ts`). A sensitive step creates an `Approval`, sets the run
  `AWAITING_APPROVAL`, and stops.
- Approve/Reject: `POST /api/runs/[id]/approvals/[stepIndex]`. Approve re-enqueues;
  the run resumes at the **same** `cursor` (the gated step), now executing it
  because its approval exists. Reject fails the run.
- UI: `apps/web/src/components/run-timeline.tsx` polls `GET /api/runs/[id]` every
  1.5s and renders status, screenshots, verification, and the approve/reject
  control.

## Known gaps / rough edges to expect

- **DB-backed job is unproven at runtime** (see step 1). Watch for Prisma enum/
  Json typing mismatches, the `runId_index` compound key on `RunStep`, and the
  `approval → resume → execute-the-gated-step` cursor logic specifically.
- **No DB-backed integration test yet.** The hermetic Playwright test
  (`apps/worker/src/browser/driver.test.ts`) covers the browser layer only. Add a
  worker integration test that runs `runWorkflowJob` against a test Postgres
  (Testcontainers or the docker-compose DB) through the gate→approve→finish path.
- **Polling, not push.** The timeline polls; upgrading to SSE/WebSocket is a nice
  follow-up but not required.
- **No visual step editor.** Workflows are created via the seed "demo" endpoint
  only. Building `workflows/new` + `[id]` with a typed editor (validate with the
  `@ghost/core` `workflowStep` Zod schema) is the main remaining Phase 1 UI.
- **Connector steps (`apiCall`/`sendEmail`) are no-ops** in execution — reserved
  types. Real API execution + the `Connector`/`Credential` model is a later phase.
- **Disk artifact store only for dev.** S3 path exists (`storage/artifacts.ts`)
  but the web serve route (`/api/artifacts/[...key]`) reads disk; wire presigned
  URLs when you turn on S3.

## Next tasks, in priority order

1. **Verify the loop end-to-end** and fix what breaks (above).
2. **Worker integration test** for `runWorkflowJob` against a real Postgres
   (gate → approve → succeed; and reject → fail).
3. **Typed step editor** (`apps/web` `workflows/new` + `[id]`) so workflows can be
   authored, not just seeded. Reuse `workflowStep`/`parseWorkflowSteps` from
   `@ghost/core/schema/step`.
4. **Harden**: per-step timeouts + retry, emergency-stop button (set run
   `CANCELED`; the worker already checks it each step), audit-chain verify
   endpoint (reuse `verifyAuditChain` in `@ghost/core/audit`).
5. **Phase 2 — recording.** Open decision: server-side remote cloud browser
   (recommended; reuses the run context + stored credentials) vs. Chrome
   extension. The engine is behind a clean boundary so either slots in.

## Repo conventions

- Validate before pushing: `pnpm typecheck && pnpm test && pnpm build` from `cloud/`.
- Every new Tauri-equivalent surface (here: API route / job) should stay
  org-scoped via `auth()` and keep the trust pipeline intact.
- CI: `cloud/ci/cloud.yml` exists but is **not active** — the automation token
  lacked GitHub `workflow` scope, so it's staged outside `.github/workflows/`.
  Move it in (`git mv cloud/ci/cloud.yml .github/workflows/`) once you have a
  token with that scope; see `cloud/ci/README.md`.
- Product docs (`CLAUDE.md`, `AGENTS.md`, `docs/*`) still describe the old
  local-first desktop product and contradict this cloud direction — a separate
  docs-realignment pass is pending.

## Current PR

Branch `claude/ghost-product-strategy-wzb4m0`, PR **#366** (draft). Phase 0
(scaffold) already merged as #365.
