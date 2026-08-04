import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * Every authenticated route group must be listed in the middleware matcher.
 *
 * `/audit` and `/recordings` both shipped without an entry. Nothing leaked —
 * `(app)/layout.tsx` redirects when there is no session — but the two guards
 * disagreeing is the kind of drift that stays invisible until the day the
 * layout stops being the thing that saves you.
 *
 * The matcher is read out of the source text rather than imported: Next.js
 * requires `config.matcher` to be an inline literal it can analyse
 * statically, so it cannot live in a shared module, and importing
 * `middleware.ts` here would drag the Auth.js runtime (and `next/server`)
 * into a plain node test.
 */
describe("middleware matcher", () => {
  const srcDir = __dirname;

  const routeSegments = readdirSync(join(srcDir, "app", "(app)"), { withFileTypes: true })
    .filter((e) => e.isDirectory())
    // Route groups `(x)` and private folders `_x` do not appear in the URL.
    .filter((e) => !e.name.startsWith("(") && !e.name.startsWith("_"))
    .map((e) => e.name)
    .sort();

  const matcherEntries = (() => {
    const source = readFileSync(join(srcDir, "middleware.ts"), "utf8");
    const block = /matcher:\s*\[([\s\S]*?)\n  \]/.exec(source);
    if (!block) throw new Error("could not find `matcher: [...]` in middleware.ts");
    // Strip `//` comments first: an apostrophe inside one ("Auth.js's") reads
    // as a string delimiter to the matcher below and yields nonsense entries.
    const withoutComments = block[1]!.replace(/\/\/[^\n]*/g, "");
    return [...withoutComments.matchAll(/"([^"]+)"/g)].map((m) => m[1]!);
  })();

  it("finds the authenticated route groups on disk", () => {
    expect(routeSegments.length).toBeGreaterThan(0);
  });

  it("parses the matcher out of middleware.ts", () => {
    expect(matcherEntries.length).toBeGreaterThan(0);
  });

  it.each(routeSegments)("protects /%s", (segment) => {
    expect(matcherEntries).toContain(`/${segment}/:path*`);
  });

  it("has no page matcher entry without a matching route on disk", () => {
    const matched = matcherEntries
      .filter((m) => !m.startsWith("/api/"))
      .map((m) => m.replace(/^\//, "").replace(/\/:path\*$/, ""))
      .sort();
    expect(matched).toEqual(routeSegments);
  });

  describe("session-authenticated API surface", () => {
    // `/api/*` is matched so the second-factor gate reaches it. Without this
    // entry MFA would protect the pages while leaving the same data readable
    // by calling the API directly with the session cookie.
    const apiEntry = matcherEntries.find((m) => m.startsWith("/api/"));

    it("is covered by the matcher", () => {
      expect(apiEntry).toBeDefined();
    });

    it.each(["auth", "agent", "mfa"])("excludes /api/%s", (excluded) => {
      // auth  — gating it prevents signing in at all.
      // agent — bearer-credential authenticated, has no session to check.
      // mfa   — how the challenge is answered; gating it deadlocks the user.
      const pattern = new RegExp(`^${apiEntry!.replace(/:path\*/, ".*")}$`);
      expect(pattern.test(`/api/${excluded}/anything`)).toBe(false);
    });

    it("still covers ordinary API routes", () => {
      const pattern = new RegExp(`^${apiEntry!.replace(/:path\*/, ".*")}$`);
      for (const path of ["/api/runs/abc", "/api/workflows", "/api/recordings/x/compile"]) {
        expect(pattern.test(path)).toBe(true);
      }
    });
  });
});
