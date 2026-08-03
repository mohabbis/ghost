# CI for Ghost Cloud

> **`cloud/` still has no CI, and activating it takes one command from a
> maintainer.** Nothing in this directory runs. GitHub executes workflows only
> from `.github/workflows/`, and the only files there are `rust.yml`,
> `security.yml`, `release.yml` and `deploy-website.yml` — none of which touch
> `cloud/`.
>
> `rust.yml` is not path-scoped, so it runs on *every* pull request. A PR that
> changes only `cloud/` still shows ~22 green checks, and **not one of them
> compiles, typechecks or tests a line of the changed code**. That is the single
> most misleading thing about this repository right now.

## Activating it

`cloud.yml` cannot be moved by automation. GitHub refuses any OAuth App or
Actions token that lacks the `workflow` scope:

```
refusing to allow an OAuth App to create or update workflow
`.github/workflows/cloud.yml` without `workflow` scope
```

A maintainer signed in normally can do it in one command:

```bash
git mv cloud/ci/cloud.yml .github/workflows/cloud.yml
git rm cloud/ci/README.md          # this file describes a solved problem
git commit -m "ci: run the cloud workflow"
git push
```

**The workflow in this directory is ready to move as-is** — the environment fix
described below is already applied to it. Do not re-derive it.

### Also worth doing at the same time

`rust.yml` should stop running on cloud-only changes. This edit needs the same
`workflow` scope, so it is described here rather than applied. Add to both the
`push` and `pull_request` triggers in `.github/workflows/rust.yml`:

```yaml
    paths-ignore:
      - "cloud/**"
      - "docs/**"
      - "public/**"
      - "**/*.md"
      - ".github/workflows/cloud.yml"
```

## What it runs

Scoped to `cloud/**` and its own path, so it never fires on Rust-only changes
and never collides with `rust.yml`. From `cloud/`: `pnpm install
--frozen-lockfile`, `db:validate`, `db:migrate:deploy`, `typecheck`, `test`,
`build`.

## Why it starts real services

Postgres and Redis are service containers, not placeholder environment
variables. About 90 of the suite's 239 tests are gated on
`Boolean(process.env.DATABASE_URL)`, which makes a URL pointing at nothing the
worst of the three options:

| `DATABASE_URL` | Guard reads | Result |
| --- | --- | --- |
| unset | no database | ~90 tests silently skip; the check is green and covers none of the engine |
| set, nothing listening | database available | tests run and fail on connection refused |
| set, Postgres running | database available | the engine is actually tested |

Two details that cost time when they were wrong:

- The database user must be **`ghost`**, not `postgres`. The wrong one fails
  with "User was denied access on the database", which reads like a missing
  migration and sends you looking in the wrong place entirely.
- `prisma migrate deploy`, not `migrate dev` — the latter is interactive and
  will offer to reset the database. That is what `db:migrate:deploy` is for.

## `GHOST_SESSION_KEY` — why this workflow would have failed on day one

An earlier version of this file did not set `GHOST_SESSION_KEY`, and moving it
into place would have gone red immediately with 16 failures.

Without the key the worker skips session capture at an approval gate, so
`Run.sessionUrl` stays null; on resume `planRestore` returns `unsafe` and the
run ends `INCIDENT`. The 16 tests in `compensateRun`, `runWorkflow` and `slots`
that assert a run reaches `SUCCEEDED` or `COMPENSATED` then fail on a *status*,
naming nothing that would point at a missing key.

Setting it in this workflow's `env:` block is necessary but **was not
sufficient**: `GHOST_SESSION_KEY` was also missing from `turbo.json`'s
`globalEnv`, and Turbo passes through only what is listed there — so it was
stripped from every task even when the environment had it. Both are now fixed;
the `turbo.json` half is already on `master`.

## Verification

The full step sequence was run locally against a freshly dropped and recreated
database: `db:validate` → `db:migrate:deploy` (all migrations from nothing) →
`typecheck` 5/5 → `test` **239 passing** → `build` 4/4. The complete product
loop (run → gate → approve → resume → verify → `SUCCEEDED`, audit chain intact)
was also driven in a real browser against that stack.

Worth re-checking that the worker suite reports 90 tests rather than 45 the
first time this runs for real; 45 means the database is not being reached and
the guard has quietly disarmed itself again.
