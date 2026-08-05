import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createLogger, serializeError } from "./logger.js";

describe("createLogger", () => {
  let logSpy: ReturnType<typeof vi.spyOn>;
  let errorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    logSpy = vi.spyOn(console, "log").mockImplementation(() => undefined);
    errorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
  });

  afterEach(() => {
    logSpy.mockRestore();
    errorSpy.mockRestore();
  });

  function lastLine(spy: ReturnType<typeof vi.spyOn>): Record<string, unknown> {
    const call = spy.mock.calls.at(-1);
    if (!call) throw new Error("expected a call");
    return JSON.parse(call[0] as string);
  }

  it("writes info and debug to stdout as a single JSON line", () => {
    createLogger("worker").info("started", { queue: "run-workflow" });
    expect(logSpy).toHaveBeenCalledTimes(1);
    expect(errorSpy).not.toHaveBeenCalled();
    const line = lastLine(logSpy);
    expect(line).toMatchObject({ level: "info", service: "worker", msg: "started", queue: "run-workflow" });
    expect(typeof line.time).toBe("string");
  });

  it("writes warn and error to stderr, not stdout", () => {
    createLogger("worker").error("job failed", { jobId: "1" });
    expect(logSpy).not.toHaveBeenCalled();
    expect(errorSpy).toHaveBeenCalledTimes(1);
    expect(lastLine(errorSpy)).toMatchObject({ level: "error", msg: "job failed", jobId: "1" });
  });

  it("child() merges bound fields without mutating the parent", () => {
    const base = createLogger("worker");
    const child = base.child({ runId: "r1" });

    child.info("halted");
    expect(lastLine(logSpy)).toMatchObject({ runId: "r1", msg: "halted" });

    base.info("unrelated");
    expect(lastLine(logSpy)).not.toHaveProperty("runId");
  });

  it("per-call fields win over bound fields with the same key", () => {
    createLogger("worker", { runId: "bound" }).info("msg", { runId: "override" });
    expect(lastLine(logSpy).runId).toBe("override");
  });
});

describe("serializeError", () => {
  it("pulls name/message/stack off an Error, which spreading alone would drop", () => {
    const err = new TypeError("bad input");
    const fields = serializeError(err);
    expect(fields.errorName).toBe("TypeError");
    expect(fields.errorMessage).toBe("bad input");
    expect(typeof fields.stack).toBe("string");
  });

  it("wraps a non-Error thrown value rather than losing it", () => {
    expect(serializeError("plain string")).toEqual({ error: "plain string" });
    expect(serializeError({ code: 42 })).toEqual({ error: { code: 42 } });
  });
});
