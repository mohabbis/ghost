import { prisma, Prisma } from "./db.js";
import { hashAuditEvent, type AuditPayload } from "./audit.js";

/**
 * Transactional writers for the two hash chains.
 *
 * Both appends are read-head-then-insert, which is a lost-update race: two
 * writers read the same head and both insert, forking the chain. The previous
 * implementation (inline in the worker) had exactly this bug, and ordered by
 * `createdAt`, which can tie. Two changes fix it:
 *
 *   1. a Postgres transaction-scoped advisory lock serializes appends per chain
 *      (released on commit/rollback — nothing to clean up, no rows locked);
 *   2. a monotonic `seq` with a unique constraint, so any path that forgets the
 *      lock fails loudly instead of forking silently.
 *
 * This lives in core rather than the worker because the web app appends too —
 * an approval decision belongs in the tamper-evident log, and two separate
 * implementations would re-introduce the fork.
 */

/** Advisory-lock namespaces, so a run chain and an org chain never collide. */
const ORG_CHAIN = "ghost:audit:org";
const RUN_CHAIN = "ghost:audit:run";

export type Tx = Prisma.TransactionClient;

/**
 * Raised when an append observes a different tail from the one committed on
 * Run. Refusing the append preserves the evidence instead of blessing a
 * truncated journal with a new expected head.
 */
export class RunJournalIntegrityError extends Error {
  constructor(runId: string, expected: string | null, actual: string | null) {
    super(
      `JOURNAL_TAMPERED: run ${runId} journal tail does not match its expected head (expected ${expected ?? "empty"}, actual ${actual ?? "empty"})`,
    );
    this.name = "RunJournalIntegrityError";
  }
}

/**
 * Raised when an append observes a different tail from the one committed on
 * Organization. Same shape as RunJournalIntegrityError, one level up: refusing
 * the append preserves the evidence instead of building the org's next event
 * on top of a chain someone deleted a suffix of.
 */
export class OrgAuditChainIntegrityError extends Error {
  constructor(orgId: string, expected: string | null, actual: string | null) {
    super(
      `AUDIT_CHAIN_TAMPERED: org ${orgId} audit chain tail does not match its expected head (expected ${expected ?? "empty"}, actual ${actual ?? "empty"})`,
    );
    this.name = "OrgAuditChainIntegrityError";
  }
}

/**
 * The exact payload shape that gets hashed.
 *
 * Writer and verifier must agree byte-for-byte, so the payload is normalized
 * here and a row read back from the database reproduces it exactly. Without
 * this, a payload written with `metadata` absent reads back as `metadata: null`
 * and `canonicalize` produces a different string — making an intact chain
 * verify as broken.
 */
export function normalizeAuditPayload(input: {
  action: string;
  entityType: string;
  entityId?: string | null;
  metadata?: unknown;
}): AuditPayload {
  return {
    action: input.action,
    entityType: input.entityType,
    entityId: input.entityId ?? null,
    metadata: input.metadata ?? null,
  };
}

/** Rebuild the hashed payload from a stored row. Inverse of the write path. */
export function auditPayloadFromRow(row: {
  action: string;
  entityType: string;
  entityId: string | null;
  metadata: unknown;
}): AuditPayload {
  return normalizeAuditPayload(row);
}

/** Rebuild the hashed payload from a stored RunEvent row. */
export function runEventPayloadFromRow(row: {
  type: string;
  runId: string;
  stepIndex: number | null;
  payload: unknown;
}): AuditPayload {
  return normalizeAuditPayload({
    action: row.type,
    entityType: "Run",
    entityId: row.runId,
    metadata: { stepIndex: row.stepIndex ?? null, payload: row.payload ?? null },
  });
}

async function lock(tx: Tx, namespace: string, key: string): Promise<void> {
  await tx.$executeRaw`SELECT pg_advisory_xact_lock(hashtextextended(${`${namespace}:${key}`}, 0))`;
}

function asJson(value: unknown): Prisma.InputJsonValue | typeof Prisma.JsonNull {
  return value === undefined || value === null
    ? Prisma.JsonNull
    : (value as Prisma.InputJsonValue);
}

/**
 * Append one event to an organization's chain.
 *
 * Pass `tx` to join a caller's transaction — that is what lets a step's
 * RunStep row, its cursor, and its journal entry commit atomically.
 */
export async function appendAuditEvent(
  orgId: string,
  actorId: string | null,
  input: { action: string; entityType: string; entityId?: string | null; metadata?: unknown },
  tx?: Tx,
): Promise<{ seq: number; hash: string }> {
  const run = async (client: Tx) => {
    await lock(client, ORG_CHAIN, orgId);

    const head = await client.auditEvent.findFirst({
      where: { orgId },
      orderBy: { seq: "desc" },
      select: { seq: true, hash: true },
    });
    const expected = await client.organization.findUniqueOrThrow({
      where: { id: orgId },
      select: { auditChainHead: true },
    });
    const actualHead = head?.hash ?? null;
    if (actualHead !== expected.auditChainHead) {
      throw new OrgAuditChainIntegrityError(orgId, expected.auditChainHead, actualHead);
    }

    const payload = normalizeAuditPayload(input);
    const prevHash = head?.hash ?? null;
    const hash = hashAuditEvent(prevHash, payload);
    const seq = (head?.seq ?? 0) + 1;

    await client.auditEvent.create({
      data: {
        orgId,
        actorId,
        action: payload.action,
        entityType: payload.entityType,
        entityId: payload.entityId ?? null,
        metadata: asJson(payload.metadata),
        seq,
        prevHash,
        hash,
      },
    });

    await client.organization.update({ where: { id: orgId }, data: { auditChainHead: hash } });

    return { seq, hash };
  };

  return tx ? run(tx) : prisma.$transaction(run);
}

/**
 * Append one event to a run's journal.
 *
 * Per-run rather than per-org so concurrent runs never contend on a single
 * chain tail. The expected head is also stored on Run in this transaction,
 * because internal links alone cannot detect deletion of a valid suffix.
 */
export async function appendRunEvent(
  runId: string,
  event: { type: string; stepIndex?: number | null; payload?: unknown },
  tx?: Tx,
): Promise<{ seq: number; hash: string }> {
  const run = async (client: Tx) => {
    await lock(client, RUN_CHAIN, runId);

    const head = await client.runEvent.findFirst({
      where: { runId },
      orderBy: { seq: "desc" },
      select: { seq: true, hash: true },
    });
    const expected = await client.run.findUniqueOrThrow({
      where: { id: runId },
      select: { journalHead: true },
    });
    const actualHead = head?.hash ?? null;
    if (actualHead !== expected.journalHead) {
      throw new RunJournalIntegrityError(runId, expected.journalHead, actualHead);
    }

    const stepIndex = event.stepIndex ?? null;
    const payload = runEventPayloadFromRow({
      type: event.type,
      runId,
      stepIndex,
      payload: event.payload ?? null,
    });
    const prevHash = head?.hash ?? null;
    const hash = hashAuditEvent(prevHash, payload);
    const seq = (head?.seq ?? 0) + 1;

    await client.runEvent.create({
      data: {
        runId,
        seq,
        stepIndex,
        type: event.type,
        payload: asJson(event.payload),
        prevHash,
        hash,
      },
    });

    await client.run.update({ where: { id: runId }, data: { journalHead: hash } });

    return { seq, hash };
  };

  return tx ? run(tx) : prisma.$transaction(run);
}

/** The current head hash of a run's journal, or null if it has no events. */
export async function runChainHead(runId: string, tx?: Tx): Promise<string | null> {
  const client = tx ?? prisma;
  const head = await client.runEvent.findFirst({
    where: { runId },
    orderBy: { seq: "desc" },
    select: { hash: true },
  });
  return head?.hash ?? null;
}
