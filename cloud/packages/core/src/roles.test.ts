import { describe, expect, it } from "vitest";
import { canApproveRun, canPublishWorkflow, canStartRun, isOrgAdmin } from "./roles.js";

describe("isOrgAdmin", () => {
  it("treats OWNER and ADMIN as admin roles", () => {
    expect(isOrgAdmin("OWNER")).toBe(true);
    expect(isOrgAdmin("ADMIN")).toBe(true);
  });

  it("does not treat MEMBER as an admin role", () => {
    expect(isOrgAdmin("MEMBER")).toBe(false);
  });
});

describe("capability matrix", () => {
  it("lets only admins publish and approve", () => {
    for (const role of ["OWNER", "ADMIN"] as const) {
      expect(canPublishWorkflow(role)).toBe(true);
      expect(canApproveRun(role)).toBe(true);
      expect(canStartRun(role)).toBe(true);
    }
    expect(canPublishWorkflow("MEMBER")).toBe(false);
    expect(canApproveRun("MEMBER")).toBe(false);
    expect(canStartRun("MEMBER")).toBe(true);
  });
});
