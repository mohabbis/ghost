import { describe, expect, it } from "vitest";
import { isOrgAdmin } from "./roles.js";

describe("isOrgAdmin", () => {
  it("treats OWNER and ADMIN as admin roles", () => {
    expect(isOrgAdmin("OWNER")).toBe(true);
    expect(isOrgAdmin("ADMIN")).toBe(true);
  });

  it("does not treat MEMBER as an admin role", () => {
    expect(isOrgAdmin("MEMBER")).toBe(false);
  });
});
