# Ghost

[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

> **Ghost is an AI operator** — teach it a business workflow once; it executes
> that workflow across the software you already use, with human approval on
> sensitive actions, verification of outcomes, and a full audit log.

```text
Capture → Review → Approve → Execute → Verify → Recover
```

Prefer, in order: **APIs → browser automation → desktop automation → vision
fallback.** AI may propose; deterministic code executes only approved plans.
Ghost is not a generic autonomous agent.

**Who it's for:** operations-heavy SMBs — wholesale distributors, property
managers, accounting/bookkeeping firms, logistics, recruiting, financial-ops and
healthcare-admin teams — anyone with recurring, measurable work that spans
several systems and can't afford silent mistakes.

## Current product: Ghost Cloud (`cloud/`)

The active product is a **cloud SaaS** in [`cloud/`](cloud/) — Next.js + Node
worker + Postgres + Redis + Playwright.

| MVP pillar | Status |
|---|---|
| Record a browser workflow | Phase 2 |
| Convert recording → editable steps | Phase 2 |
| Replay across browser / API | **Phase 1 — built** |
| Approval before sensitive actions | **Phase 1 — built** |
| Log every run (screenshots, verify, audit) | **Phase 1 — built** |

```bash
cd cloud
cp .env.example .env          # set AUTH_SECRET, GHOST_ARTIFACT_DIR, APP_URL
pnpm install
docker compose up -d          # Postgres + Redis
pnpm db:migrate
pnpm --filter @ghost/worker exec playwright install chromium
pnpm dev                      # http://localhost:3000
```

Smoke test: sign in → **Workflows → Create demo workflow → Run** → approve
"Submit order" → run **SUCCEEDED**.

Docs: [`cloud/README.md`](cloud/README.md) ·
[`cloud/docs/PHASE_1_PLAN.md`](cloud/docs/PHASE_1_PLAN.md) ·
[`cloud/docs/CURSOR_HANDOFF.md`](cloud/docs/CURSOR_HANDOFF.md) ·
[`AGENTS.md`](AGENTS.md).

Validate: `cd cloud && pnpm typecheck && pnpm test && pnpm build`.

## Trust pipeline

Every meaningful operation:

```text
Intent → Plan → Policy check → User approval → Execution → Audit log → Undo path
```

- Deny risky actions by default (send, pay, delete, submit).
- Sensitive steps halt for a human; the classifier is deterministic, not AI.
- Runs append to a per-org hash-chained audit log.
- Connectors use scoped, revocable, least-privilege credentials — never around
  the approval + audit pipeline.

## Legacy desktop app (superseded, retained)

The Rust/Tauri app at the repo root (`src-tauri/`, `src/`, `apps/macos/`) is the
previous local-first product (Ghost Organizer, record/replay). It remains in-tree
for reference and maintenance but is **not** the current product direction.

Published desktop release: **[v2.0.3](https://github.com/mohabbis/ghost/releases/tag/v2.0.3)**
(macOS notarized; Windows unsigned). See [`RELEASING.md`](RELEASING.md) and
desktop build notes in [`AGENTS.md`](AGENTS.md) if you need that tree.

## What Ghost is not

- Not a generic autonomous AI agent or chat wrapper.
- Not a no-code Zapier clone or blind macro recorder.
- Not silent — capture and sensitive execution are explicit, interruptible, and audited.

## License

MIT License — see [LICENSE](LICENSE).
