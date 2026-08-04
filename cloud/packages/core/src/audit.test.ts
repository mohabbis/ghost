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

  it("detects a deleted event in the middle of the chain", () => {
    const chain = buildChain(payloads);
    // Excise event 1. Event 2's prevHash now points at a hash no longer
    // preceding it, so the break surfaces at the new index 1.
    const tampered = [chain[0]!, chain[2]!];
    const result = verifyAuditChain(tampered);
    expect(result.intact).toBe(false);
    expect(result.firstBreakIndex).toBe(1);
  });

  it("detects two events swapped into the wrong order", () => {
    const chain = buildChain(payloads);
    const tampered = [chain[0]!, chain[2]!, chain[1]!];
    const result = verifyAuditChain(tampered);
    expect(result.intact).toBe(false);
    expect(result.firstBreakIndex).toBe(1);
  });

  it("detects a forged event spliced into the chain", () => {
    const chain = buildChain(payloads);
    const forged = {
      prevHash: chain[0]!.hash,
      hash: hashAuditEvent(chain[0]!.hash, {
        action: "approval.approved",
        entityType: "Approval",
        entityId: "r1:0:FORWARD",
      }),
      payload: {
        action: "approval.approved",
        entityType: "Approval",
        entityId: "r1:0:FORWARD",
      } satisfies AuditPayload,
    };
    // The forged event hashes correctly against event 0, so it is self
    // consistent — but it displaces event 1, whose prevHash no longer matches.
    const tampered = [chain[0]!, forged, chain[1]!, chain[2]!];
    const result = verifyAuditChain(tampered);
    expect(result.intact).toBe(false);
    expect(result.firstBreakIndex).toBe(2);
  });

  // ---------------------------------------------------------------------
  // What the chain deliberately does NOT catch.
  //
  // These two tests assert `intact: true` on tampered input. That is not an
  // oversight being enshrined — it is the boundary of what a bare hash chain
  // can prove, pinned so nobody markets it as more. See
  // docs/why-deterministic-gates.md ("What the audit chain does not prove").
  // ---------------------------------------------------------------------

  it("does NOT detect truncation of the tail — the chain has no end marker", () => {
    const chain = buildChain(payloads);
    // Drop the final event, e.g. to hide that a run ever completed. Every
    // surviving link still verifies, because nothing commits to the length.
    const truncated = chain.slice(0, 2);
    expect(verifyAuditChain(truncated)).toEqual({ intact: true, firstBreakIndex: null });
    // This function only ever sees the array it's handed, so it cannot catch
    // this by itself — the mitigation lives one layer up, in appendRunEvent /
    // appendAuditEvent (auditLog.test.ts): Run.journalHead and
    // Organization.auditChainHead each store the expected tail out-of-band, so
    // the next append raises RunJournalIntegrityError / OrgAuditChainIntegrityError
    // instead of building on a chain someone truncated. Both are still same-
    // database columns, so an attacker able to rewrite the chain rows and that
    // column together remains outside what either catches — see the next test.
  });

  it("does NOT detect a wholesale rewrite by someone who can write every row", () => {
    // An attacker with database write access rewrites history and recomputes
    // every hash forward from the change. The result is a perfectly intact
    // chain of events that never happened.
    const rewritten = buildChain([
      payloads[0]!,
      { action: "step.forged", entityType: "RunStep", entityId: "r1:0" },
      payloads[2]!,
    ]);
    expect(verifyAuditChain(rewritten)).toEqual({ intact: true, firstBreakIndex: null });
    // A hash chain proves internal consistency, not authenticity. Detecting
    // this requires an anchor the attacker cannot recompute: a signed head, a
    // WORM/append-only replica, or periodic publication of the head hash.
  });
});
