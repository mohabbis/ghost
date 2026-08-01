import { createHash } from "node:crypto";

/**
 * Append-only audit hash chain. Each event's hash folds in the previous event's
 * hash plus a canonical serialization of this event's payload, so any tampering
 * with historical rows breaks the chain from that point forward.
 *
 * `AuditEvent.hash = sha256(prevHash + canonical(payload))`.
 */

export interface AuditPayload {
  action: string;
  entityType: string;
  entityId?: string | null;
  actorId?: string | null;
  metadata?: unknown;
}

/** Deterministic JSON: object keys sorted recursively. */
export function canonicalize(value: unknown): string {
  return JSON.stringify(sortDeep(value));
}

function sortDeep(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortDeep);
  if (value && typeof value === "object") {
    return Object.keys(value as Record<string, unknown>)
      .sort()
      .reduce<Record<string, unknown>>((acc, key) => {
        acc[key] = sortDeep((value as Record<string, unknown>)[key]);
        return acc;
      }, {});
  }
  return value;
}

export function hashAuditEvent(prevHash: string | null, payload: AuditPayload): string {
  return createHash("sha256")
    .update(prevHash ?? "")
    .update(canonicalize(payload))
    .digest("hex");
}

/** Verify a chain of `{ prevHash, hash, payload }` rows in order. */
export interface ExpectedChainTail {
  /** Number of rows that must exist in the complete chain. */
  seq: number;
  /** Hash of the final row, or null for an empty chain. */
  hash: string | null;
}

/**
 * Verify the links and hashes in a chain, optionally against an independently
 * persisted expected tail. Link verification alone cannot detect deletion of a
 * complete suffix: the surviving prefix is still internally valid.
 */
export function verifyAuditChain(
  events: Array<{ prevHash: string | null; hash: string; payload: AuditPayload }>,
  expectedTail?: ExpectedChainTail,
): { intact: boolean; firstBreakIndex: number | null } {
  let expectedPrev: string | null = null;
  for (let i = 0; i < events.length; i++) {
    const e = events[i]!;
    if (e.prevHash !== expectedPrev) return { intact: false, firstBreakIndex: i };
    if (hashAuditEvent(e.prevHash, e.payload) !== e.hash) {
      return { intact: false, firstBreakIndex: i };
    }
    expectedPrev = e.hash;
  }

  if (
    expectedTail &&
    (events.length !== expectedTail.seq || expectedPrev !== expectedTail.hash)
  ) {
    // A missing suffix breaks immediately after the last surviving row. An
    // unexpected extra suffix also reports at the expected boundary.
    return {
      intact: false,
      firstBreakIndex: Math.min(events.length, expectedTail.seq),
    };
  }

  return { intact: true, firstBreakIndex: null };
}
