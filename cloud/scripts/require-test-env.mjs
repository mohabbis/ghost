#!/usr/bin/env node
/**
 * Fail loudly when the env that DB-gated tests need is missing.
 *
 * Without this, `pnpm test` exits 0 while ~100 tests — every test of the
 * run / approval / verify loop — silently skip via `describe.skipIf(!hasDb)`.
 * That is a green signal covering nothing of the safety-critical half of the
 * product. CI always sets these; a local developer who has not started
 * Postgres/Redis should see a clear refusal rather than a false pass.
 *
 * Escape hatch: `GHOST_ALLOW_SKIP_DB_TESTS=1` restores the old skip behaviour
 * for hermetic unit-only iteration. Prefer `pnpm demo` / docker compose.
 */

const required = ["DATABASE_URL", "REDIS_URL", "GHOST_SESSION_KEY"];
const missing = required.filter((k) => !process.env[k]?.trim());

if (missing.length === 0) process.exit(0);

if (process.env.GHOST_ALLOW_SKIP_DB_TESTS === "1") {
  console.warn(
    `[ghost] GHOST_ALLOW_SKIP_DB_TESTS=1 — continuing without ${missing.join(", ")}; DB-gated tests will skip.`,
  );
  process.exit(0);
}

console.error(
  `[ghost] pnpm test requires ${missing.join(", ")} so the run/approval/verify suite actually executes.`,
);
console.error(
  "  Start Postgres + Redis (`pnpm demo` or `docker compose up -d`), then copy cloud/.env.example → cloud/.env.",
);
console.error(
  "  To run unit-only tests and knowingly skip the DB suite: GHOST_ALLOW_SKIP_DB_TESTS=1 pnpm test",
);
process.exit(1);
