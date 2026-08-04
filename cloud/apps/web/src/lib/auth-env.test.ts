import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  assertAuthSecretUsable,
  devSignInAllowed,
  isLoopbackInstance,
  PUBLISHED_DEV_AUTH_SECRET,
  sessionMaxAgeSeconds,
} from "@/lib/auth-env";

/**
 * Both behaviours here fail silently when they regress — the app boots,
 * sign-in works, and the only symptom is that sessions are forgeable or
 * anyone can sign in as anyone. So they get explicit tests.
 */

const ENV_KEYS = [
  "NODE_ENV",
  "NEXT_PHASE",
  "AUTH_SECRET",
  "AUTH_URL",
  "NEXTAUTH_URL",
  "GHOST_DISABLE_DEV_SIGNIN",
  "GHOST_SESSION_MAX_AGE_SECONDS",
] as const;

// `NODE_ENV` is typed readonly, but varying it is the whole point here.
const env = process.env as Record<string, string | undefined>;

let saved: Record<string, string | undefined>;

beforeEach(() => {
  saved = Object.fromEntries(ENV_KEYS.map((k) => [k, env[k]]));
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
});

afterEach(() => {
  for (const k of ENV_KEYS) {
    if (saved[k] === undefined) delete env[k];
    else env[k] = saved[k];
  }
  vi.restoreAllMocks();
});

function setEnv(patch: Partial<Record<(typeof ENV_KEYS)[number], string | undefined>>) {
  for (const [k, v] of Object.entries(patch)) {
    if (v === undefined) delete env[k];
    else env[k] = v;
  }
}

describe("assertAuthSecretUsable", () => {
  it("refuses to start in production with the published example secret", () => {
    setEnv({ NODE_ENV: "production", AUTH_SECRET: PUBLISHED_DEV_AUTH_SECRET });
    expect(() => assertAuthSecretUsable()).toThrow(/published example value/);
  });

  it("refuses to start in production with no secret", () => {
    setEnv({ NODE_ENV: "production", AUTH_SECRET: undefined });
    expect(() => assertAuthSecretUsable()).toThrow(/AUTH_SECRET is not set/);
  });

  it("refuses to start in production with a too-short secret", () => {
    setEnv({ NODE_ENV: "production", AUTH_SECRET: "short" });
    expect(() => assertAuthSecretUsable()).toThrow(/at least 32/);
  });

  it("accepts a real secret in production", () => {
    setEnv({ NODE_ENV: "production", AUTH_SECRET: "y".repeat(44) });
    expect(() => assertAuthSecretUsable()).not.toThrow();
  });

  it("only warns outside production, so dev keeps working", () => {
    setEnv({ NODE_ENV: "development", AUTH_SECRET: PUBLISHED_DEV_AUTH_SECRET });
    expect(() => assertAuthSecretUsable()).not.toThrow();
    expect(console.warn).toHaveBeenCalled();
  });

  it("does not fail `next build`, which runs as production without secrets", () => {
    // `next build` imports every route module with NODE_ENV=production to
    // collect page data. Asserting there would make a real secret a
    // prerequisite for compiling, including in CI.
    setEnv({
      NODE_ENV: "production",
      NEXT_PHASE: "phase-production-build",
      AUTH_SECRET: undefined,
    });
    expect(() => assertAuthSecretUsable()).not.toThrow();
  });

  it("still asserts when serving in production", () => {
    setEnv({ NODE_ENV: "production", NEXT_PHASE: undefined, AUTH_SECRET: undefined });
    expect(() => assertAuthSecretUsable()).toThrow(/AUTH_SECRET is not set/);
  });
});

describe("devSignInAllowed", () => {
  it("is off in production even on localhost", () => {
    setEnv({ NODE_ENV: "production", AUTH_URL: "http://localhost:3000" });
    expect(devSignInAllowed()).toBe(false);
  });

  it("is off on a deployed host even when NODE_ENV is unset", () => {
    // The whole point of the second signal: NODE_ENV is routinely missing.
    setEnv({ NODE_ENV: undefined, AUTH_URL: "https://ghost.example.com" });
    expect(devSignInAllowed()).toBe(false);
  });

  it("is off when no AUTH_URL is set at all", () => {
    setEnv({ NODE_ENV: "development", AUTH_URL: undefined, NEXTAUTH_URL: undefined });
    expect(devSignInAllowed()).toBe(false);
  });

  it("is off when explicitly disabled", () => {
    setEnv({
      NODE_ENV: "development",
      AUTH_URL: "http://localhost:3000",
      GHOST_DISABLE_DEV_SIGNIN: "true",
    });
    expect(devSignInAllowed()).toBe(false);
  });

  it("is on for local development", () => {
    setEnv({
      NODE_ENV: "development",
      AUTH_URL: "http://localhost:3000",
      GHOST_DISABLE_DEV_SIGNIN: undefined,
    });
    expect(devSignInAllowed()).toBe(true);
  });
});

describe("isLoopbackInstance", () => {
  it("treats an unparseable URL as exposed", () => {
    setEnv({ AUTH_URL: "not a url" });
    expect(isLoopbackInstance()).toBe(false);
  });

  it("recognises 127.0.0.1", () => {
    setEnv({ AUTH_URL: "http://127.0.0.1:3000" });
    expect(isLoopbackInstance()).toBe(true);
  });
});

describe("sessionMaxAgeSeconds", () => {
  it("defaults to 12 hours rather than Auth.js's 30 days", () => {
    setEnv({ GHOST_SESSION_MAX_AGE_SECONDS: undefined });
    expect(sessionMaxAgeSeconds()).toBe(12 * 60 * 60);
  });

  it("honours a valid override and ignores nonsense", () => {
    setEnv({ GHOST_SESSION_MAX_AGE_SECONDS: "3600" });
    expect(sessionMaxAgeSeconds()).toBe(3600);
    setEnv({ GHOST_SESSION_MAX_AGE_SECONDS: "-5" });
    expect(sessionMaxAgeSeconds()).toBe(12 * 60 * 60);
  });
});
