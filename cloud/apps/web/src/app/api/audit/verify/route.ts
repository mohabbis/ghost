import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
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

  const events = await prisma.auditEvent.findMany({
    where: { orgId },
    orderBy: { seq: "asc" },
  });
  const org = verifyAuditChain(
    events.map((e) => ({ prevHash: e.prevHash, hash: e.hash, payload: auditPayloadFromRow(e) })),
  );

  const body: Record<string, unknown> = {
    org: {
      intact: org.intact,
      firstBreakIndex: org.firstBreakIndex,
      count: events.length,
      headHash: events.at(-1)?.hash ?? null,
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
