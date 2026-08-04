import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { rateLimit, tooManyRequests } from "@/lib/rate-limit";
import { verifyAuditChain } from "@ghost/core/audit";
import { auditPayloadFromRow, runEventPayloadFromRow } from "@ghost/core/audit-log";

/**
 * Verify the organization's audit chain, and optionally one run's journal.
 *
 * `verifyAuditChain` has existed and been unit-tested since Phase 1 but was
 * never reachable from anywhere — an audit log nobody can check is a log, not
 * a proof. This is the endpoint that lets a customer (or their auditor) ask the
 * question directly.
 *
 * A broken chain is reported, not hidden. If an org's history was forked by the
 * old non-transactional appender, this will say so, which is the right outcome:
 * an audit log that quietly conceals a break is worse than one that admits it.
 */
export async function GET(req: Request) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const runId = new URL(req.url).searchParams.get("runId");

  // Verification is inherently whole-chain — a hash chain cannot be checked in
  // pages — so this is the most expensive read in the product and it grows
  // with the tenant's entire history. Rate limited per organization because a
  // loop over it is a one-request-each denial of service against the web tier,
  // and because it is a deliberate audit action rather than something a UI
  // polls. Six a minute is generous for a human and useless as an attack.
  //
  // This bounds the blast radius; it does not fix the underlying cost. The
  // unbounded findMany below is P1-1 in docs/ARCHITECTURE_DECISIONS.md and
  // needs incremental verification with a checkpoint, not a limiter.
  const limited = await rateLimit(`audit-verify:${orgId}`, { limit: 6, windowSeconds: 60 });
  if (!limited.ok) return tooManyRequests(limited);

  const events = await prisma.auditEvent.findMany({
    where: { orgId },
    orderBy: { seq: "asc" },
  });
  const chain = verifyAuditChain(
    events.map((e) => ({ prevHash: e.prevHash, hash: e.hash, payload: auditPayloadFromRow(e) })),
  );

  // Internal hash links alone cannot detect deletion of a valid suffix — the
  // surviving prefix still verifies as intact, just shorter. auditChainHead is
  // the expected tail persisted on Organization outside AuditEvent itself (see
  // appendAuditEvent), so a suffix deleted without also rewriting that column
  // is caught here instead of silently accepted as "the chain that happens to
  // exist now".
  const orgRow = await prisma.organization.findUniqueOrThrow({
    where: { id: orgId },
    select: { auditChainHead: true },
  });
  const orgHead = events.at(-1)?.hash ?? null;
  const orgExpectedHeadMatches = orgHead === orgRow.auditChainHead;

  const body: Record<string, unknown> = {
    org: {
      intact: chain.intact && orgExpectedHeadMatches,
      firstBreakIndex: chain.firstBreakIndex,
      count: events.length,
      headHash: orgHead,
      expectedHeadHash: orgRow.auditChainHead,
      expectedHeadMatches: orgExpectedHeadMatches,
    },
  };

  if (runId) {
    const owned = await prisma.run.findFirst({
      where: { id: runId, orgId },
      select: { id: true, journalHead: true },
    });
    if (!owned) return NextResponse.json({ error: "not found" }, { status: 404 });

    const runEvents = await prisma.runEvent.findMany({
      where: { runId },
      orderBy: { seq: "asc" },
    });
    const chain = verifyAuditChain(
      runEvents.map((e) => ({
        prevHash: e.prevHash,
        hash: e.hash,
        payload: runEventPayloadFromRow(e),
      })),
    );

    // The org chain records each finished run's journal head. If the two
    // disagree, the journal was altered after the run sealed it.
    const head = runEvents.at(-1)?.hash ?? null;
    const expectedHeadMatches = head === owned.journalHead;
    const anchors = events
      .filter((e) => e.entityId === runId && e.metadata && typeof e.metadata === "object")
      .map((e) => (e.metadata as { runChainHead?: unknown }).runChainHead)
      .filter((h): h is string => typeof h === "string");

    body.run = {
      intact: chain.intact && expectedHeadMatches,
      firstBreakIndex: chain.firstBreakIndex,
      count: runEvents.length,
      headHash: head,
      expectedHeadHash: owned.journalHead,
      expectedHeadMatches,
      anchored: anchors.length > 0,
      anchorMatches: anchors.length === 0 ? null : anchors.includes(head ?? ""),
    };
  }

  return NextResponse.json(body);
}
