#!/usr/bin/env bash
#
# Shared setup logic for `pnpm demo` and `pnpm doctor`.
#
# The one idea in this file: **never conclude a service is usable because a port
# is open.** The original demo script tested `port_open 5432` and, on any
# machine with an unrelated Postgres installed — Homebrew's, Postgres.app,
# another project's container — declared success, wrote that port into .env, and
# handed the user an app whose every database call was denied. The failure
# surfaced later as unexplained 500s, so the setup step that caused it looked
# like the one thing that had gone right.
#
# So each check here actually speaks the protocol: Postgres is probed by running
# a query as the `ghost` user, Redis by sending a PING and reading the PONG.

# --- output ----------------------------------------------------------------

bold()  { printf '\033[1m%s\033[0m\n' "$1"; }
info()  { printf '  \033[36m→\033[0m %s\n' "$1"; }
ok()    { printf '  \033[32m✓\033[0m %s\n' "$1"; }
warn()  { printf '  \033[33m!\033[0m %s\n' "$1"; }
fail()  { printf '  \033[31m✗\033[0m %s\n' "$1"; }
die()   { fail "$1"; exit 1; }

# --- ports -----------------------------------------------------------------

port_open() { (exec 3<>"/dev/tcp/127.0.0.1/$1") >/dev/null 2>&1; }

# --- .env ------------------------------------------------------------------

# Read a single value out of .env, stripping surrounding quotes.
env_get() {
  local key="$1"
  [ -f .env ] || return 1
  sed -n "s/^${key}=//p" .env | head -1 | sed -e 's/^"//' -e 's/"$//'
}

# Set (or add) a key in .env. Portable across BSD and GNU sed.
env_set() {
  local key="$1" value="$2"
  touch .env
  if grep -q "^${key}=" .env; then
    # `|` as the delimiter, and the value escaped, so a URL's slashes and any
    # `&` in a password cannot corrupt the file being repaired.
    local escaped
    escaped="$(printf '%s' "$value" | sed -e 's/[\\|&]/\\&/g')"
    sed -i.bak "s|^${key}=.*|${key}=\"${escaped}\"|" .env
    rm -f .env.bak
  else
    printf '%s="%s"\n' "$key" "$value" >> .env
  fi
}

# --- service probes --------------------------------------------------------

pg_url_for_port() { printf 'postgresql://ghost:ghost@localhost:%s/ghost?schema=public' "$1"; }

# True when a real Postgres answers a query as the ghost user on this URL.
pg_usable() {
  local url="$1"
  echo 'SELECT 1;' | pnpm --filter @ghost/core exec prisma db execute --stdin --url "$url" \
    >/dev/null 2>&1
}

# True when Redis answers PING. Inline commands are valid RESP input, so this
# needs no client — and unlike an open port, a PONG proves it is Redis.
redis_usable() {
  local port="$1" reply=""
  exec 3<>"/dev/tcp/127.0.0.1/${port}" 2>/dev/null || return 1
  printf 'PING\r\n' >&3
  # `read -t` bounds the wait: a port held open by something that never
  # answers must fail the check rather than hang the setup.
  IFS= read -r -t 3 reply <&3 || true
  exec 3<&- 3>&-
  case "$reply" in *PONG*) return 0 ;; *) return 1 ;; esac
}

port_of_url() {
  # Pulls the port out of scheme://user:pass@host:PORT/db, empty if absent.
  printf '%s' "$1" | sed -n 's|.*://[^/]*:\([0-9][0-9]*\)/.*|\1|p'
}

# --- data plane ------------------------------------------------------------

# Find a Postgres that actually accepts the ghost credentials, starting Docker
# only if none does. Echoes the chosen port.
#
# Candidates in order: whatever .env already says (so a deliberate choice is
# kept), the compose default, then the fallback the compose file uses when 5432
# is taken. Trying the configured value first also means a working setup
# re-runs with no changes at all.
resolve_pg_port() {
  local configured candidates=() c
  configured="$(port_of_url "$(env_get DATABASE_URL || true)")"
  [ -n "$configured" ] && candidates+=("$configured")
  candidates+=(5432 55432)

  for c in "${candidates[@]}"; do
    if pg_usable "$(pg_url_for_port "$c")"; then
      printf '%s' "$c"
      return 0
    fi
  done
  return 1
}

resolve_redis_port() {
  local configured candidates=() c
  configured="$(port_of_url "$(env_get REDIS_URL || true)")"
  [ -n "$configured" ] && candidates+=("$configured")
  candidates+=(6379 56379)

  for c in "${candidates[@]}"; do
    if redis_usable "$c"; then
      printf '%s' "$c"
      return 0
    fi
  done
  return 1
}

docker_available() { command -v docker >/dev/null && docker info >/dev/null 2>&1; }

# Bring up Postgres and Redis on ports that are genuinely free, and write those
# ports into .env. Sets PG_PORT and REDIS_PORT for the caller.
ensure_data_plane() {
  PG_PORT="$(resolve_pg_port || true)"
  REDIS_PORT="$(resolve_redis_port || true)"

  if [ -n "$PG_PORT" ] && [ -n "$REDIS_PORT" ]; then
    ok "Postgres :$PG_PORT and Redis :$REDIS_PORT answering as ghost"
  else
    docker_available || die "Need Postgres and Redis.
     Start Docker and re-run, or point DATABASE_URL and REDIS_URL in cloud/.env
     at instances you already have. A Postgres that is merely running is not
     enough — it must accept user 'ghost' on database 'ghost'."

    # Bind to a port nothing else holds. Publishing onto an occupied port fails
    # the whole compose run, and reusing one occupied by a *different* Postgres
    # is the bug this file exists to prevent.
    if [ -z "$PG_PORT" ]; then
      GHOST_PG_PORT=5432
      port_open 5432 && GHOST_PG_PORT=55432
      export GHOST_PG_PORT
      info "starting Postgres on :$GHOST_PG_PORT"
    fi
    if [ -z "$REDIS_PORT" ]; then
      GHOST_REDIS_PORT=6379
      port_open 6379 && GHOST_REDIS_PORT=56379
      export GHOST_REDIS_PORT
      info "starting Redis on :$GHOST_REDIS_PORT"
    fi

    docker compose up -d >/dev/null

    local i
    for i in $(seq 1 45); do
      [ -n "$PG_PORT" ] || PG_PORT="$(pg_usable "$(pg_url_for_port "${GHOST_PG_PORT:-5432}")" && echo "${GHOST_PG_PORT:-5432}" || true)"
      [ -n "$REDIS_PORT" ] || REDIS_PORT="$(redis_usable "${GHOST_REDIS_PORT:-6379}" && echo "${GHOST_REDIS_PORT:-6379}" || true)"
      [ -n "$PG_PORT" ] && [ -n "$REDIS_PORT" ] && break
      sleep 1
    done

    [ -n "$PG_PORT" ] || die "Postgres did not accept the ghost user on :${GHOST_PG_PORT:-5432}.
     Check: docker compose logs postgres"
    [ -n "$REDIS_PORT" ] || die "Redis did not answer PING on :${GHOST_REDIS_PORT:-6379}.
     Check: docker compose logs redis"
    ok "Postgres :$PG_PORT and Redis :$REDIS_PORT are up"
  fi

  # Write back what actually worked, repairing a stale .env rather than leaving
  # the app pointed at a database that denies it.
  local want_db want_redis
  want_db="$(pg_url_for_port "$PG_PORT")"
  want_redis="redis://localhost:${REDIS_PORT}"
  [ "$(env_get DATABASE_URL || true)" = "$want_db" ] || { env_set DATABASE_URL "$want_db"; ok "pointed DATABASE_URL at :$PG_PORT"; }
  [ "$(env_get REDIS_URL || true)" = "$want_redis" ] || { env_set REDIS_URL "$want_redis"; ok "pointed REDIS_URL at :$REDIS_PORT"; }
}
