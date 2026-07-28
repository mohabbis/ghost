# Ghost Cloud — handoff for Cursor

You're picking up **Ghost Cloud**, the cloud SaaS rebuild of Ghost: an AI
operator that learns a business workflow once, then executes it across existing
software with **human approval on sensitive actions, verification, and a full
audit log**. All new work lives under `cloud/` (a self-contained pnpm +
Turborepo workspace). The legacy Rust/Tauri desktop app at the repo root is
out of scope unless explicitly requested.

Read `cloud/README.md` and `cloud/docs/PHASE_1_PLAN.md` first — this file is the
"where we are / what to do next" layer on top of them.

## Where we are

Phase 0 + Phase 1.1 + Phase 1.2 are **merged on `master`**. The DB-backed
demo loop has been executed end-to-end:

```text
sign-in → create demo workflow → run → AWAITING_APPROVAL on Submit
  → Approve → prefix restore → submit → verify → SUCCEEDED
```

Reject path fails the run without submitting.

### Bug fixed: resume after approval

Ephemeral Chromium is closed when the worker halts at a gate. On resume the
job used to start on `about:blank` and time out looking for "Submit order".
`restoreBrowserPrefix` in `apps/worker/src/browser/driver.ts` replays
`steps[0..cursor)` before continuing. Covered by:

- `apps/worker/src/browser/driver.test.ts` (hermetic restore)
- `apps/worker/src/jobs/runWorkflow.test.ts` (Postgres gate → approve → succeed;
  reject → fail) — runs when `DATABASE_URL` is set, otherwise skipped

## Local loop

```bash
cd cloud
cp .env.example .env
#  In .env set at minimum:
#    AUTH_SECRET   -> `openssl rand -base64 32`
#    GHOST_ARTIFACT_DIR -> ABSOLUTE path both web+worker share
#    APP_URL       -> http://localhost:3000
pnpm install
docker compose up -d
pnpm db:migrate
pnpm --filter @ghost/worker exec playwright install chromium
pnpm dev   # or separate web / worker filters
```

Then: sign in (any email) → Workflows → Create demo workflow → Run → Approve.

## Architecture

```
cloud/
  packages/core/         Prisma, Zod workflow-step schema, classifyStep, audit chain
  apps/web/              Next.js 15 + Tailwind v4 + Auth.js v5
  apps/worker/           BullMQ + Playwright execution engine
```

**Trust model (do not regress):** AI may propose; deterministic code executes
approved plans. The approval gate is enforced in
`apps/worker/src/runtime/state-machine.ts` via `@ghost/core`'s `classifyStep` —
no AI in that decision. Every run/step/gate/finish appends to a per-org
hash-chained `AuditEvent`.

**Execution flow:**

- `POST /api/runs {workflowId}` → `Run` (QUEUED) + enqueue `runWorkflow`
- Worker walks steps from `run.cursor` via `planNextAction`. Sensitive step
  with no APPROVED approval → create `Approval`, `AWAITING_APPROVAL`, stop
  (browser closed).
- `POST /api/runs/[id]/approvals/[stepIndex]` approve → re-enqueue; worker
  restores prefix, executes the gated step, continues. Reject → `FAILED`.
- UI: `run-timeline.tsx` polls every 1.5s.

## Remaining Phase 1 work (priority)

1. **Typed step editor** (`workflows/new` + `[id]`) — author steps, not only
   the demo seed. Validate with `@ghost/core` `workflowStep` Zod schema.
2. **Harden** — per-step timeouts + retry; emergency-stop UI (set `CANCELED`;
   worker already checks each step); `GET` audit-chain verify using
   `verifyAuditChain` in `@ghost/core/audit`.
3. **SSE/WebSocket** for the timeline (nice-to-have; polling works).
4. **S3 serve path** — disk store works in dev; wire presigned URLs when S3 is on.

## Phase 2

Recording → editable workflow. Open decision: server-side remote cloud browser
(recommended) vs Chrome extension. Engine boundary is clean either way.

## Repo conventions

- Validate from `cloud/`: `pnpm typecheck && pnpm test && pnpm build`.
- Pass `DATABASE_URL` (and a migrated DB) for the worker integration tests.
- API routes / jobs stay org-scoped via `auth()`; keep the trust pipeline intact.
- CI: `cloud/ci/cloud.yml` is staged outside `.github/workflows/` until a token
  with `workflow` scope can move it (see `cloud/ci/README.md`).
- Canonical product docs: `AGENTS.md`, `CLAUDE.md`, `cloud/README.md`. Root
  `README.md` should describe the cloud product; the desktop app is legacy.
