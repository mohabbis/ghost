import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { config as loadDotenv } from "dotenv";

/**
 * Loads `cloud/.env` into `process.env`.
 *
 * The README has always said `cp .env.example .env` and then `pnpm dev`, and
 * that could not work: `.env` lives at the workspace root while the apps run
 * from `apps/web` and `apps/worker`. Next.js reads `.env` relative to its own
 * project directory and the worker read nothing at all, so the worker died on
 * "REDIS_URL is not set" and the web app ran against whatever happened to be
 * exported in the shell that started it. Every local setup that appeared to
 * work was one where someone had exported the variables by hand — which is
 * also why the failure never reproduced for whoever had done so.
 *
 * Import this **first**, before any module that reads `process.env` at import
 * time. ES module evaluation follows import order, so a first import is
 * enough.
 *
 * Deliberately non-overriding: a real environment (Vercel, a container,
 * `DATABASE_URL=... pnpm dev`) always wins over the file. In deployment there
 * is no `.env` and this is a no-op.
 */

/** Walks up from `startDir` looking for the workspace `.env`. */
export function findEnvFile(startDir: string = process.cwd()): string | null {
  let dir = resolve(startDir);
  // Bounded by reaching the filesystem root, where `dirname` is a fixed point.
  for (;;) {
    const candidate = join(dir, ".env");
    if (existsSync(candidate)) return candidate;
    // Stop at the workspace root rather than escaping into the user's home
    // directory and picking up an unrelated project's .env.
    if (existsSync(join(dir, "pnpm-workspace.yaml"))) return null;
    const parent = dirname(dir);
    if (parent === dir) return null;
    dir = parent;
  }
}

let loaded: string | null | undefined;

/** Returns the path loaded, or null when there was no file to load. */
export function loadEnv(startDir?: string): string | null {
  if (loaded !== undefined && startDir === undefined) return loaded;
  const file = findEnvFile(startDir);
  if (file) loadDotenv({ path: file, override: false });
  if (startDir === undefined) loaded = file;
  return file;
}

loadEnv();
