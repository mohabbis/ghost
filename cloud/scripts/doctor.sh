#!/usr/bin/env bash
#
# Ghost Cloud — say plainly what is wrong.
#
#   cd cloud && pnpm doctor
#
# Read-only. It changes nothing; `pnpm demo` is what repairs. This exists
# because the way Ghost breaks locally is almost never a visible error: the web
# app renders fine while pointed at a database that denies it, or while no
# worker is running to execute anything, so "Run" simply does nothing forever.
# Both are one-line diagnoses that were previously invisible.
set -uo pipefail

cd "$(dirname "$0")/.."
ROOT="$PWD"
# shellcheck source=scripts/lib.sh
source "$ROOT/scripts/lib.sh"

PROBLEMS=0
note_problem() { PROBLEMS=$((PROBLEMS + 1)); }

bold "Ghost Cloud doctor"
echo

# --- config ----------------------------------------------------------------
if [ -f .env ]; then
  ok ".env present"
else
  fail ".env missing — run: pnpm demo"
  note_problem
  exit 1
fi

ARTIFACT_DIR="$(env_get GHOST_ARTIFACT_DIR || true)"
case "$ARTIFACT_DIR" in
  /*) [ -d "$ARTIFACT_DIR" ] && ok "GHOST_ARTIFACT_DIR $ARTIFACT_DIR" || {
        fail "GHOST_ARTIFACT_DIR points at $ARTIFACT_DIR, which does not exist"
        note_problem
      } ;;
  "") fail "GHOST_ARTIFACT_DIR is empty — run screenshots will 404, so approvals show no evidence"
      note_problem ;;
  *)  fail "GHOST_ARTIFACT_DIR must be an absolute path (got '$ARTIFACT_DIR')"
      note_problem ;;
esac

SESSION_KEY="$(env_get GHOST_SESSION_KEY || true)"
if [ -z "$SESSION_KEY" ]; then
  warn "GHOST_SESSION_KEY is empty — approved runs will end INCIDENT, not SUCCEEDED"
  note_problem
else
  ok "GHOST_SESSION_KEY set"
fi

# --- data plane ------------------------------------------------------------
DB_URL="$(env_get DATABASE_URL || true)"
DB_PORT="$(port_of_url "$DB_URL")"
if [ -z "$DB_URL" ]; then
  fail "DATABASE_URL is not set"
  note_problem
elif pg_usable "$DB_URL"; then
  ok "Postgres :$DB_PORT accepts the ghost user"
else
  fail "Postgres on :$DB_PORT is not usable by Ghost."
  if port_open "${DB_PORT:-5432}"; then
    fail "  Something IS listening there — most likely a different Postgres"
    fail "  (Homebrew, Postgres.app, another project). An open port is not enough."
  else
    fail "  Nothing is listening on :$DB_PORT."
  fi
  fail "  Fix: pnpm demo   (it finds a port that actually works and repairs .env)"
  note_problem
fi

REDIS_URL="$(env_get REDIS_URL || true)"
REDIS_PORT="$(port_of_url "$REDIS_URL")"
: "${REDIS_PORT:=6379}"
if redis_usable "$REDIS_PORT"; then
  ok "Redis :$REDIS_PORT answers PING"
else
  fail "Redis on :$REDIS_PORT did not answer PING — runs can be created but never queued"
  fail "  Fix: pnpm demo"
  note_problem
fi

# --- processes -------------------------------------------------------------
if port_open 3000; then
  ok "web app listening on :3000"
else
  warn "nothing on :3000 — the web app is not running (pnpm dev)"
  note_problem
fi

# The worker binds no port, so presence is judged by process, and by whether
# anything it should have consumed is still sitting in the queue.
if pgrep -f "@ghost/worker|worker/dist/index.js|worker/src/index.ts" >/dev/null 2>&1; then
  ok "worker process running"
else
  fail "NO WORKER RUNNING. Runs will stay PENDING forever — nothing executes."
  fail "  Fix: pnpm dev   (starts web and worker together)"
  fail "  Or:  pnpm --filter @ghost/worker dev"
  note_problem
fi

echo
if [ "$PROBLEMS" -eq 0 ]; then
  ok "no problems found"
else
  bold "$PROBLEMS problem(s) above."
fi
exit 0
