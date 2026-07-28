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
export function verifyAuditChain(
  events: Array<{ prevHash: string | null; hash: string; payload: AuditPayload }>,
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
  return { intact: true, firstBreakIndex: null };
}
