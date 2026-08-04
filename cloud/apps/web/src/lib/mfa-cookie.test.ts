import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { signMfaCookie, verifyMfaCookie } from "@/lib/mfa-cookie";

/**
 * This cookie is the only thing that satisfies the second-factor gate, so a
 * forgery-accepting bug here silently removes 2FA for everyone while the UI
 * still shows it as "On".
 */

const SECRET = "test-secret-at-least-32-characters-long!!";
let saved: string | undefined;

beforeEach(() => {
  saved = process.env.AUTH_SECRET;
  process.env.AUTH_SECRET = SECRET;
});
afterEach(() => {
  if (saved === undefined) delete process.env.AUTH_SECRET;
  else process.env.AUTH_SECRET = saved;
});

describe("MFA cookie", () => {
  it("round-trips for the user it was issued to", async () => {
    const { value } = await signMfaCookie("user-1", "sid-1");
    expect(await verifyMfaCookie(value, "user-1", "sid-1")).toBe(true);
  });

  it("is bound to one user", async () => {
    const { value } = await signMfaCookie("user-1", "sid-1");
    expect(await verifyMfaCookie(value, "user-2", "sid-1")).toBe(false);
  });

  it("rejects a tampered payload", async () => {
    const { value } = await signMfaCookie("user-1", "sid-1");
    const [body, sig] = value.split(".");
    // Re-encode a payload naming a different user, keeping the old signature.
    const forged = Buffer.from(
      JSON.stringify({ uid: "user-2", sid: "sid-1", exp: Math.floor(Date.now() / 1000) + 600 }),
      "utf8",
    )
      .toString("base64url")
      .replace(/=+$/, "");
    expect(body).toBeDefined();
    expect(await verifyMfaCookie(`${forged}.${sig}`, "user-2", "sid-1")).toBe(false);
  });

  it("rejects a signature made with a different secret", async () => {
    const { value } = await signMfaCookie("user-1", "sid-1");
    process.env.AUTH_SECRET = "a-completely-different-secret-value-32ch";
    expect(await verifyMfaCookie(value, "user-1", "sid-1")).toBe(false);
  });

  it("does not survive a sign-out and a fresh sign-in", async () => {
    // The regression this exists for: bound to the user alone, a cookie
    // minted when the factor was proved kept satisfying the gate after a
    // sign-out — so someone holding only the password could sign in on that
    // machine and never be challenged. A new sign-in mints a new `sid`.
    const { value } = await signMfaCookie("user-1", "sid-before-signout");
    expect(await verifyMfaCookie(value, "user-1", "sid-before-signout")).toBe(true);
    expect(await verifyMfaCookie(value, "user-1", "sid-after-signin")).toBe(false);
  });

  it("rejects a cookie when the session carries no id at all", async () => {
    const { value } = await signMfaCookie("user-1", "sid-1");
    expect(await verifyMfaCookie(value, "user-1", undefined)).toBe(false);
  });

  it("rejects an expired cookie", async () => {
    const { value } = await signMfaCookie("user-1", "sid-1", -1);
    expect(await verifyMfaCookie(value, "user-1", "sid-1")).toBe(false);
  });

  it("returns false rather than throwing on junk", async () => {
    for (const junk of [undefined, "", "nodot", "a.b", "....", "%%%.%%%"]) {
      expect(await verifyMfaCookie(junk, "user-1", "sid-1")).toBe(false);
    }
  });
});
