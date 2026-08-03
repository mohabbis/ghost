# Contributing to Ghost

Thanks for looking at Ghost. This document is the short path in: what the repo
contains, how to get a working stack, the two setup traps that will otherwise
cost you an hour, and the invariants a change must not break.

If you only read one section, read [Trust invariants](#trust-invariants). Those
are the rules the product exists to enforce, and a PR that breaks one will be
sent back however good the code is.

---

## What is in this repository

Two products share the tree, and only one is alive.

| Path | What | Status |
|---|---|---|
| `cloud/` | Ghost Cloud — Next.js + Node worker + Postgres + Redis + Playwright | **Active. Work here.** |
| `src-tauri/`, `src/`, `apps/macos/` | Legacy Rust/Tauri desktop app | Superseded, retained for reference |
| `public/` | Marketing site | Deployed to Vercel |
| `docs/` | Product/architecture docs (`docs/legacy/` = desktop only) | — |

Unless you are deliberately maintaining the old desktop app, **everything you
want is in `cloud/`**, and every command below is run from there.

---

## Getting a working stack

Prerequisites: Node >= 20 (developed on 22), pnpm 10 (`corepack enable`), and
Docker for local Postgres + Redis.

```bash
cd cloud
cp .env.example .env      # works as-is for local dev; see the traps below
pnpm install
docker compose up -d      # Postgres :5432, Redis :6379
pnpm db:migrate
pnpm --filter @ghost/worker exec playwright install chromium
pnpm dev                  # web on http://localhost:3000, plus the worker
```

Then sign in with any email (the dev credentials provider needs no mail server),
go to **Workflows → Create demo workflow → Run**, and approve the "Submit order"
gate. The run should reach **SUCCEEDED** with a screenshot per step and a
verified audit chain. If it does, your environment is correct.

To validate a change:

```bash
pnpm typecheck && pnpm test && pnpm build
```

A full green run is **239 tests** (core 127, worker 90, web 21, mcp 1). If you
see substantially fewer, read on — you are almost certainly hitting one of these
two traps.

### Trap 1: `DATABASE_URL` decides whether ~90 tests run at all

About 90 tests are gated on `Boolean(process.env.DATABASE_URL)` and **skip
silently** without it. This produces the worst possible outcome: a completely
green test run that exercised none of the execution engine.

| `DATABASE_URL` | What happens |
|---|---|
| unset | ~90 tests silently skip; green, and covers nothing |
| set, nothing listening | tests run and fail on connection refused |
| set, Postgres running | the engine is actually tested |

**Check the worker suite reports 90 tests, not 45.** 45 means the database is
not being reached and the guard has quietly disarmed itself.

### Trap 2: `GHOST_SESSION_KEY` is required, and fails confusingly

When a run halts at an approval gate, the worker encrypts and stores the
browser's `storageState` so the resumed run can rebuild the page without
replaying any prior action. With no key configured, that capture is skipped,
`Run.sessionUrl` stays null, and on resume `planRestore` returns `unsafe` — so
**every approved run ends `INCIDENT` instead of `SUCCEEDED`**.

Refusing to replay unverifiable actions is correct production behaviour. But it
means an empty key breaks the one flow Ghost exists to demonstrate, and 16 tests
fail asserting a run status rather than anything naming the key.

`.env.example` now ships a dev-only throwaway value so `cp .env.example .env`
just works. **Generate a real one for anything that touches real systems:**

```bash
openssl rand -base64 32   # must decode to exactly 32 bytes
```

If you add a new environment variable, add it to `cloud/turbo.json` as well —
Turbo passes through only what is listed there, so an unlisted variable is
invisible to anything run via `pnpm test` / `pnpm build` even when it is set in
your shell.

---

## Trust invariants

Ghost's value is not that it automates things; it is that it can be trusted to.
These properties are the product. A change that weakens one needs to say so
explicitly in the PR and have a very good reason.

1. **AI proposes; deterministic code executes.** The decision to gate an action
   is a pure function (`classifyStep`), never a model call. AI may draft or
   explain a workflow; it may not decide what is safe.
2. **Sensitive actions are deny-by-default.** Send, pay, delete, submit and
   friends halt for a human. Adding a step type means deciding its
   classification — `classifyStep`'s `switch` is exhaustive on purpose, and the
   `default` branch fails closed.
3. **Agents may propose, never approve.** No agent-facing surface may gain an
   approve or reject verb.
4. **What the human approved is what runs.** Template references resolve
   *before* classification and before the approval preview. Any transformation
   applied after a human's decision is an unreviewed change.
5. **At-most-once steps are never retried or auto-resumed.** A timed-out
   "Submit order" is missing information, not a failure. Indeterminate outcomes
   raise an incident for a human rather than being guessed.
6. **Run position is derived from the journal, never stored as a cursor.** This
   is what makes crash recovery safe; do not "optimize" the linear scan in
   `planNextAction` back into a stored position.
7. **Every run and mutation appends to the hash chain.** Do not add a path that
   mutates customer systems without an audit event.
8. **Secrets never enter logs, screenshots, audit payloads, or model prompts.**
   Run event payloads carry no fill values, no `storageState`, no cookies.
9. **Tenant isolation.** Every query is scoped to an organization.
   Cross-tenant reachability is a bug, not a feature request.
10. **Docs must not promise what the code cannot do.** If you change behaviour,
    change the docs in the same PR.

Background on why several of these are shaped the way they are, including the
bug that motivated #6: [`docs/why-deterministic-gates.md`](docs/why-deterministic-gates.md).

---

## Making a change

- **Branch from `master`.** Keep changes scoped; avoid drive-by rewrites.
- **Match the surrounding code.** This codebase comments *why*, not *what*, and
  explains non-obvious trade-offs where they live. A comment explaining why a
  linear scan is deliberate is worth more than one restating the loop.
- **Tests for behaviour, not coverage.** The most valuable tests here pin a
  trust property — that a gate cannot be skipped, that a reversal is
  re-entrant, that tampering is detected. `audit.test.ts` also has tests that
  assert the chain's *limits*; that pattern is encouraged.
- **Run the checks.** `pnpm typecheck && pnpm test && pnpm build` from `cloud/`.
  CI runs the same against real Postgres and Redis services.
- **Say what you did not verify.** Claiming a check that did not run is worse
  than admitting a gap.

### Pull requests

The PR template is a layout to fill in. Beyond it:

- Describe the behaviour change and the reasoning, not just the diff.
- Call out any trust invariant the change touches.
- If you changed a documented behaviour, update the doc in the same PR.

### Working on the legacy desktop app

Only if you specifically need to. It is Rust/Tauri, has its own checks
(`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, all via
`make ci` from the repo root), and its own docs under `docs/legacy/`. CI does
not run an `--features experimental` leg, so run those locally and say so in
the PR.

---

## Reporting bugs and security issues

- **Bugs and features:** open an issue using one of the templates in
  `.github/ISSUE_TEMPLATE/`.
- **Security:** do not open a public issue. Follow
  [`SECURITY.md`](SECURITY.md).

---

## Licence

Ghost is **AGPL-3.0-or-later**. By contributing you agree your contributions are
licensed under it. In practice: use it, modify it, self-host it — but if you run
a modified Ghost as a network service other people use, section 13 requires you
to offer those users the source of the version you are running.
