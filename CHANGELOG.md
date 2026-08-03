# Changelog

All notable changes to Ghost are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Ghost Cloud has not cut a numbered release yet — it is pre-1.0 and the sections
below track the work by date. The legacy desktop app's releases are versioned
separately and listed at the bottom.

## [Unreleased]

### Fixed

- **The cloud CI workflow is ready to activate, and would have failed on its
  first run.** `cloud/ci/cloud.yml` was written and verified but lives outside
  `.github/workflows/`, which GitHub never reads — so the active product has no
  CI at all. It also did not set `GHOST_SESSION_KEY`, so moving it as written
  would have gone red with 16 failures; that is now fixed in the file. Moving it
  requires a token with the `workflow` scope, which automation does not have —
  `cloud/ci/README.md` has the one-command handoff, plus the `rust.yml`
  path-scoping that needs the same scope.
- **`pnpm test` no longer fails 16 tests out of the box.** `GHOST_SESSION_KEY`
  was absent from `turbo.json`'s environment allow-list, so Turbo stripped it
  from every task even when it was set in the shell. Without it the worker skips
  session capture at an approval gate and every approved run ends `INCIDENT`
  instead of `SUCCEEDED`. Added to `turbo.json` and to the CI environment, and
  `.env.example` now ships a dev-only default so `cp .env.example .env` yields a
  working stack.

- **`pnpm db:migrate` now works from a clean checkout.** The `db:*` scripts
  delegated to `pnpm --filter @ghost/core`, which runs with `packages/core` as
  its working directory — where there is no `.env`. Prisma does not search
  parent directories, so the documented setup (`cp .env.example .env` then
  `pnpm db:migrate`) failed with `P1012: Environment variable not found:
  DATABASE_URL` unless you happened to have exported the variables yourself.
  The scripts now invoke Prisma from `cloud/` with an explicit `--schema`, so
  `cloud/.env` is picked up.

### Added

- **`pnpm demo`** — one command from a fresh clone to a running Ghost: config,
  dependencies, Postgres + Redis (reusing whatever is already listening, else
  Docker), schema, Chromium, then the dev servers, ending with the four clicks
  that demonstrate the approval gate. Idempotent, so it also repairs a
  half-configured checkout.
- Adversarial tests for the audit hash chain: deletion of a middle event,
  reordering, and a spliced forged event. Two further tests deliberately assert
  `intact: true` on tampered input, pinning what a bare hash chain **cannot**
  prove — tail truncation and a wholesale rewrite by anyone with write access.
- `docs/why-deterministic-gates.md` — why the approval gate is a pure function
  rather than a model call, worked through the double-payment retry clamp, the
  indeterminate at-most-once step, resolve-before-classify, and the audit
  chain's limits.
- `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, and this changelog.
- Screenshots of the real approval gate, verified run, and audit chain in the
  READMEs.

### Changed

- The legacy Rust/Tauri tree is marked `linguist-vendored` so GitHub reports the
  repository as TypeScript — the language of the active product — rather than
  Rust, which is four times the line count but superseded. (Path-scoping
  `rust.yml` so its 3-OS matrix stops running on cloud-only changes needs the
  `workflow` scope; the exact edit is in `cloud/ci/README.md`.)

---

## Ghost Cloud

### 2026-08-03

- **Separation of duties** — `Workflow.requireSeparateApprover` prevents the
  person who started a run from approving it. Off by default, because every org
  is currently single-member. Only approval is restricted; rejection stays open
  to whoever started the run.

### 2026-08-02

- **Deployability** — shared artifact store, run screenshots served from S3, so
  web and worker can run on separate hosts without every screenshot 404ing.
- **Working CI for `cloud/`** (see Unreleased — it was not installed until now),
  and runs that never started no longer appear queued.
- **Typed step editor** — author a workflow by hand and run it.
- Chromium is resolved where the browser is launched rather than per entrypoint.
- **Licensed AGPL-3.0-or-later.**

### 2026-08-01

- **Truncated run journals are detected before resume** and refused rather than
  blessed with a new head.
- **Concurrency cap** — a limit on how many runs of one workflow may be in
  flight at once.
- **Compensation** — reverse a completed run's side effects in reverse order,
  with the approval gate intact.
- **Durable execution** — run position is derived by folding the run journal
  instead of a stored cursor, closing a double-submit hole where a redelivered
  job re-ran an already-approved payment.

### 2026-07-28

- **Agent surface** — Ghost Cloud agent HTTP API and MCP bridge. Agents may
  propose and start runs; they cannot approve. Plus a Ghost-authenticated Claude
  Code plugin.
- **Phase 1 engine** — Playwright execution with a deterministic approval gate,
  run trigger, approval resume, and a live timeline UI. Phase 1 E2E loop proven
  end to end.
- Ghost Cloud SaaS scaffolded (Phase 0).

---

## Legacy desktop app (superseded)

The Rust/Tauri desktop product remains in-tree for reference. Its last published
release is **[v2.0.3](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)**
(macOS notarized; Windows unsigned) — treat that binary as a legacy preview, not
the current product. Highlights of that line included the Ghost Organizer trust
pipeline (policy engine, planner, executor, undo journal, hash-chained audit),
Ghost Guard, on-device OCR and ID parsing, the Action Plan runtime, and a stable
MCP pairing/approval surface.

Ghost Cloud supersedes it. The trust pipeline carried forward; the local-first
delivery model did not.
