import { describe, expect, it } from "vitest";
import {
  screenshotKey,
  restoreScreenshotKey,
  sessionStateKey,
  sessionPrefix,
  servableRunId,
} from "./artifacts.js";

const RUN = "run_abc123";

describe("artifact keys", () => {
  it("builds the expected key shapes", () => {
    expect(screenshotKey(RUN, 2)).toBe("runs/run_abc123/step-2.png");
    expect(restoreScreenshotKey(RUN, 2)).toBe("runs/run_abc123/restore-2.png");
    expect(sessionStateKey(RUN, 2)).toBe("runs/run_abc123/session/gate-2.bin");
    expect(sessionPrefix(RUN)).toBe("runs/run_abc123/session");
  });
});

describe("servableRunId", () => {
  it("serves step and restore screenshots", () => {
    expect(servableRunId(screenshotKey(RUN, 0).split("/"))).toBe(RUN);
    expect(servableRunId(restoreScreenshotKey(RUN, 12).split("/"))).toBe(RUN);
  });

  it("REFUSES the encrypted session blob", () => {
    // The whole reason this is an allow-list. The session blob holds live
    // session cookies for the customer's systems; a deny-list that only
    // rejected ".." would hand it to any member of the run's org.
    expect(servableRunId(sessionStateKey(RUN, 2).split("/"))).toBeNull();
    expect(servableRunId(["runs", RUN, "session"])).toBeNull();
  });

  it("refuses traversal and injection", () => {
    expect(servableRunId(["runs", "..", "step-0.png"])).toBeNull();
    expect(servableRunId(["runs", RUN, "../../../etc/passwd"])).toBeNull();
    expect(servableRunId(["runs", RUN, "step-0.png\0.txt"])).toBeNull();
    expect(servableRunId(["runs", RUN, ".", "step-0.png"])).toBeNull();
  });

  it("refuses anything that is not the exact shape", () => {
    expect(servableRunId(["runs", RUN])).toBeNull();
    expect(servableRunId(["runs", RUN, "step-0.png", "extra"])).toBeNull();
    expect(servableRunId(["other", RUN, "step-0.png"])).toBeNull();
    expect(servableRunId(["runs", "", "step-0.png"])).toBeNull();
    expect(servableRunId(["runs", RUN, "step-.png"])).toBeNull();
    expect(servableRunId(["runs", RUN, "step-0.svg"])).toBeNull();
    expect(servableRunId(["runs", RUN, "notes.txt"])).toBeNull();
  });
});
