#!/usr/bin/env bash
#
# Ghost Cloud — one-command demo.
#
#   cd cloud && pnpm demo
#
# Brings a stranger from a fresh clone to a running Ghost in one step: config,
# dependencies, data plane, schema, browser, then the dev servers. Every step is
# idempotent, so re-running it is safe and is also the fastest way to repair a
# half-configured checkout.
#
# This is the local path. For deploying web + worker properly, see docs/DEPLOY.md.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$PWD"

bold()  { printf '\033[1m%s\033[0m\n' "$1"; }
info()  { printf '  \033[36m→\033[0m %s\n' "$1"; }
ok()    { printf '  \033[32m✓\033[0m %s\n' "$1"; }
warn()  { printf '  \033[33m!\033[0m %s\n' "$1"; }
die()   { printf '  \033[31m✗\033[0m %s\n' "$1" >&2; exit 1; }

bold "Ghost Cloud demo"
echo

# --- 1. Toolchain -----------------------------------------------------------
command -v node >/dev/null || die "Node >= 20 is required (https://nodejs.org)."
NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]')"
[ "$NODE_MAJOR" -ge 20 ] || die "Node >= 20 required; found $(node -v)."
command -v pnpm >/dev/null || die "pnpm is required. Run: corepack enable"
ok "node $(node -v), pnpm $(pnpm -v)"

# --- 2. Configuration -------------------------------------------------------
# .env.example is a working local config, including a dev-only GHOST_SESSION_KEY.
# Without that key every approved run ends INCIDENT instead of SUCCEEDED, which
# is precisely the flow this demo exists to show.
if [ -f .env ]; then
  ok ".env already present (left untouched)"
else
  cp .env.example .env
  ok "created .env from .env.example"
fi

# GHOST_ARTIFACT_DIR must be an absolute path shared by web and worker: the
# worker writes run screenshots there and the web app serves them. Left empty,
# each process falls back to its own cwd and every screenshot 404s — so the
# person at an approval gate would be deciding with no evidence.
if grep -q '^GHOST_ARTIFACT_DIR=""' .env 2>/dev/null; then
  ARTIFACTS="$ROOT/.artifacts"
  mkdir -p "$ARTIFACTS"
  # Portable in-place edit (BSD sed on macOS needs the empty -i argument).
  sed -i.bak "s|^GHOST_ARTIFACT_DIR=\"\"|GHOST_ARTIFACT_DIR=\"$ARTIFACTS\"|" .env
  rm -f .env.bak
  ok "set GHOST_ARTIFACT_DIR to $ARTIFACTS"
fi
mkdir -p "$ROOT/.artifacts"

# --- 3. Data plane ----------------------------------------------------------
# Reuse whatever is already listening; only reach for Docker if nothing is.
port_open() { (exec 3<>"/dev/tcp/127.0.0.1/$1") >/dev/null 2>&1; }

if port_open 5432 && port_open 6379; then
  ok "Postgres :5432 and Redis :6379 already reachable"
elif command -v docker >/dev/null && docker info >/dev/null 2>&1; then
  info "starting Postgres + Redis via docker compose"
  docker compose up -d
  for _ in $(seq 1 30); do
    port_open 5432 && port_open 6379 && break
    sleep 1
  done
  port_open 5432 || die "Postgres did not come up on :5432. Check: docker compose logs postgres"
  port_open 6379 || die "Redis did not come up on :6379. Check: docker compose logs redis"
  ok "Postgres and Redis are up"
else
  die "Need Postgres on :5432 and Redis on :6379.
     Either start Docker and re-run, or point DATABASE_URL and REDIS_URL in
     cloud/.env at instances you already have."
fi

# --- 4. Dependencies, schema, browser ---------------------------------------
info "installing dependencies"
pnpm install --silent
ok "dependencies installed"

info "applying database schema"
# `migrate deploy`, not `migrate dev` — the latter is interactive and will offer
# to reset the database.
pnpm db:migrate:deploy >/dev/null
ok "schema applied"

# Playwright's own cache is keyed by the revision it pins, so a browser
# installed for a different version does not count. This is a no-op once done.
info "ensuring Playwright Chromium is installed"
if pnpm --filter @ghost/worker exec playwright install chromium >/dev/null 2>&1; then
  ok "Chromium ready"
else
  warn "Chromium install failed — runs will not execute."
  warn "Retry manually: pnpm --filter @ghost/worker exec playwright install chromium"
fi

# --- 5. Go ------------------------------------------------------------------
cat <<'BANNER'

  Ghost is starting on http://localhost:3000

  To see the whole point of Ghost in four clicks:

    1. Sign in with any email address (no mail server needed in dev)
    2. Workflows  ->  Create demo workflow
    3. Run
    4. The run HALTS at "Approval required — Clicks a submit control".
       Approve it.

  The run then resumes from the captured browser session, verifies the
  outcome, and reaches SUCCEEDED. Every step has a screenshot; the Audit tab
  shows the hash chain covering all of it.

  Stop with Ctrl-C.

BANNER

exec pnpm dev
