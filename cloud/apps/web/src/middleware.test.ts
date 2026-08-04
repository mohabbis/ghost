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
    const block = /matcher:\s*\[([\s\S]*?)\]/.exec(source);
    if (!block) throw new Error("could not find `matcher: [...]` in middleware.ts");
    return [...block[1]!.matchAll(/["'`]([^"'`]+)["'`]/g)].map((m) => m[1]!);
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

  it("has no matcher entry without a matching route on disk", () => {
    const matched = matcherEntries
      .map((m) => m.replace(/^\//, "").replace(/\/:path\*$/, ""))
      .sort();
    expect(matched).toEqual(routeSegments);
  });
});
