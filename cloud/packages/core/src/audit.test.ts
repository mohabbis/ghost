import { describe, expect, it } from "vitest";
import { hashAuditEvent, verifyAuditChain, type AuditPayload } from "./audit.js";

function buildChain(payloads: AuditPayload[]) {
  let prev: string | null = null;
  return payloads.map((payload) => {
    const hash = hashAuditEvent(prev, payload);
    const row = { prevHash: prev, hash, payload };
    prev = hash;
    return row;
  });
}

describe("audit hash chain", () => {
  const payloads: AuditPayload[] = [
    { action: "run.started", entityType: "Run", entityId: "r1" },
    { action: "step.succeeded", entityType: "RunStep", entityId: "r1:0" },
    { action: "run.succeeded", entityType: "Run", entityId: "r1" },
  ];

  it("verifies an intact chain", () => {
    expect(verifyAuditChain(buildChain(payloads))).toEqual({
      intact: true,
      firstBreakIndex: null,
    });
  });

  it("is order-independent of object key ordering in metadata", () => {
    const a = hashAuditEvent(null, {
      action: "x",
      entityType: "E",
      metadata: { b: 2, a: 1 },
    });
    const b = hashAuditEvent(null, {
      action: "x",
      entityType: "E",
      metadata: { a: 1, b: 2 },
    });
    expect(a).toBe(b);
  });

  it("detects tampering with a historical event", () => {
    const chain = buildChain(payloads);
    // Mutate the middle event's payload without recomputing hashes.
    chain[1] = { ...chain[1]!, payload: { ...chain[1]!.payload, action: "step.forged" } };
    const result = verifyAuditChain(chain);
    expect(result.intact).toBe(false);
    expect(result.firstBreakIndex).toBe(1);
  });

  it("detects a broken prevHash link", () => {
    const chain = buildChain(payloads);
    chain[2] = { ...chain[2]!, prevHash: "deadbeef" };
    const result = verifyAuditChain(chain);
    expect(result.intact).toBe(false);
    expect(result.firstBreakIndex).toBe(2);
  });
});
