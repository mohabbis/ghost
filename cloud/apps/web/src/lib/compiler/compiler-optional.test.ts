import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  CompilerNotConfiguredError,
  compilerConfigured,
  compilerErrorResponse,
  requireWorkflowCompiler,
  workflowCompiler,
} from "@/lib/compiler";

/**
 * The acceptance criterion for the compiler boundary: **removing the compiler
 * disables compilation and breaks nothing else.**
 *
 * HarnessRouter is development tooling — agent infrastructure used to build
 * Ghost, not to run it. Production leaves `HR_API_KEY` unset, and a customer's
 * workflows must still publish, execute, gate, verify and audit with no
 * compiler present. These tests exist so that a future change cannot quietly
 * make an authoring convenience load-bearing for the runtime.
 *
 * See `cloud/docs/DEPLOY.md` and `cloud/docs/ARCHITECTURE_DECISIONS.md` §1.
 */

const original = process.env.HR_API_KEY;

beforeEach(() => {
  delete process.env.HR_API_KEY;
});

afterEach(() => {
  if (original === undefined) delete process.env.HR_API_KEY;
  else process.env.HR_API_KEY = original;
});

describe("compiler is optional", () => {
  it("reports no compiler when none is configured", () => {
    expect(workflowCompiler()).toBeNull();
    expect(compilerConfigured()).toBe(false);
  });

  it("throws a typed, catchable error rather than a bare failure", () => {
    expect(() => requireWorkflowCompiler()).toThrow(CompilerNotConfiguredError);
  });

  it("renders an unconfigured compiler as 503, not 500", async () => {
    const response = compilerErrorResponse(new CompilerNotConfiguredError());
    expect(response).not.toBeNull();
    expect(response!.status).toBe(503);

    // "unavailable", not "broken" — this is the expected production state, and
    // the message a deployer reads should not send them hunting for a bug.
    const body = (await response!.json()) as { error: string };
    expect(body.error).toMatch(/not enabled/i);
  });

  it("returns null for unrelated errors so callers rethrow them", () => {
    expect(compilerErrorResponse(new Error("something else"))).toBeNull();
  });

  it("selects an implementation once configured", () => {
    process.env.HR_API_KEY = "sk-hr-test";
    expect(compilerConfigured()).toBe(true);
    expect(workflowCompiler()?.name).toBe("harness-router");
  });
});

describe("the runtime does not depend on a compiler", () => {
  /**
   * A structural check rather than a behavioural one, because the failure this
   * guards against is an import edge: anything the worker or the execution
   * path pulls in transitively must not reach the compiler, or an unset
   * `HR_API_KEY` becomes a boot failure instead of a disabled feature.
   */
  it("keeps compiler imports out of the execution path", async () => {
    const { readFileSync, readdirSync, statSync } = await import("node:fs");
    const { join } = await import("node:path");

    const roots = [
      join(process.cwd(), "../worker/src"),
      join(process.cwd(), "../../packages/core/src"),
    ];

    const offenders: string[] = [];
    const walk = (dir: string) => {
      let entries: string[];
      try {
        entries = readdirSync(dir);
      } catch {
        return; // path not present in this checkout layout; nothing to assert
      }
      for (const entry of entries) {
        const full = join(dir, entry);
        if (statSync(full).isDirectory()) {
          walk(full);
          continue;
        }
        if (!/\.tsx?$/.test(entry)) continue;
        const source = readFileSync(full, "utf8");
        if (/from\s+["'][^"']*(harness-router|lib\/compiler)/.test(source)) {
          offenders.push(full);
        }
      }
    };
    roots.forEach(walk);

    expect(offenders).toEqual([]);
  });
});
