import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { rateLimit, tooManyRequests } from "@/lib/rate-limit";
import { verifyAuditChain } from "@ghost/core/audit";
import { auditPayloadFromRow, runEventPayloadFromRow } from "@ghost/core/audit-log";

/**
 * Verify the organization's audit chain, and optionally one run's journal.
 *
 * Two modes (query `mode=`):
 *
 *   - `head` (default for the audit page): cheap — count + last hash vs
 *     `Organization.auditChainHead`. Detects suffix deletion / head drift.
 *     Does **not** re-hash every event; mid-chain payload edits need `full`.
 *   - `full`: walks every event through `verifyAuditChain`. Caped by
 *     `GHOST_AUDIT_VERIFY_MAX_EVENTS` (default 50_000) so an unbounded
 *     tenant history cannot take the web tier down (P1-1). Over the cap → 413.
 *
 * A broken chain is reported, not hidden.
 */

const DEFAULT_MAX_EVENTS = 50_000;

function maxEvents(): number {
  const fromEnv = Number(process.env.GHOST_AUDIT_VERIFY_MAX_EVENTS);
  return Number.isFinite(fromEnv) && fromEnv > 0 ? fromEnv : DEFAULT_MAX_EVENTS;
}

export async function GET(req: Request) {
  const session = await auth();
  if (!session?.user?.orgId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const orgId = session.user.orgId;
  const url = new URL(req.url);
  const runId = url.searchParams.get("runId");
  const mode = url.searchParams.get("mode") === "full" ? "full" : "head";

  // Verification of a large chain is the most expensive read in the product.
  // Rate limited per organization so a loop over it is not a one-request-each
  // denial of service. Six a minute is generous for a human and useless as an
  // attack.
  const limited = await rateLimit(`audit-verify:${orgId}`, { limit: 6, windowSeconds: 60 });
  if (!limited.ok) return tooManyRequests(limited);

  const orgRow = await prisma.organization.findUniqueOrThrow({
    where: { id: orgId },
    select: { auditChainHead: true },
  });

  const count = await prisma.auditEvent.count({ where: { orgId } });
  const tail = await prisma.auditEvent.findFirst({
    where: { orgId },
    orderBy: { seq: "desc" },
    select: { hash: true, seq: true },
  });
  const orgHead = tail?.hash ?? null;
  const orgExpectedHeadMatches = orgHead === orgRow.auditChainHead;

  if (mode === "head") {
    const body: Record<string, unknown> = {
      mode: "head",
      org: {
        // Head mode cannot claim mid-chain integrity — only that the expected
        // tail matches what is stored on Organization.
        intact: orgExpectedHeadMatches,
        count,
        headHash: orgHead,
        expectedHeadHash: orgRow.auditChainHead,
        expectedHeadMatches: orgExpectedHeadMatches,
        fullVerifyAvailable: count <= maxEvents(),
      },
    };
    if (runId) {
      const runHead = await verifyRunHead(orgId, runId);
      if (!runHead) return NextResponse.json({ error: "not found" }, { status: 404 });
      body.run = runHead;
    }
    return NextResponse.json(body);
  }

  // Full verify — capped.
  const cap = maxEvents();
  if (count > cap) {
    return NextResponse.json(
      {
        error: "audit chain too large for a single full verify",
        count,
        maxEvents: cap,
        hint: "use mode=head for the expected-tail check, or raise GHOST_AUDIT_VERIFY_MAX_EVENTS",
      },
      { status: 413 },
    );
  }

  const events = await prisma.auditEvent.findMany({
    where: { orgId },
    orderBy: { seq: "asc" },
  });
  const chain = verifyAuditChain(
    events.map((e) => ({ prevHash: e.prevHash, hash: e.hash, payload: auditPayloadFromRow(e) })),
  );

  const body: Record<string, unknown> = {
    mode: "full",
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
    if (runEvents.length > cap) {
      return NextResponse.json(
        {
          error: "run journal too large for a single full verify",
          count: runEvents.length,
          maxEvents: cap,
        },
        { status: 413 },
      );
    }
    const runChain = verifyAuditChain(
      runEvents.map((e) => ({
        prevHash: e.prevHash,
        hash: e.hash,
        payload: runEventPayloadFromRow(e),
      })),
    );

    const head = runEvents.at(-1)?.hash ?? null;
    const expectedHeadMatches = head === owned.journalHead;
    const anchors = events
      .filter((e) => e.entityId === runId && e.metadata && typeof e.metadata === "object")
      .map((e) => (e.metadata as { runChainHead?: unknown }).runChainHead)
      .filter((h): h is string => typeof h === "string");

    body.run = {
      intact: runChain.intact && expectedHeadMatches,
      firstBreakIndex: runChain.firstBreakIndex,
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

async function verifyRunHead(orgId: string, runId: string) {
  const owned = await prisma.run.findFirst({
    where: { id: runId, orgId },
    select: { id: true, journalHead: true },
  });
  if (!owned) return null;
  const count = await prisma.runEvent.count({ where: { runId } });
  const tail = await prisma.runEvent.findFirst({
    where: { runId },
    orderBy: { seq: "desc" },
    select: { hash: true },
  });
  const head = tail?.hash ?? null;
  const expectedHeadMatches = head === owned.journalHead;
  return {
    intact: expectedHeadMatches,
    count,
    headHash: head,
    expectedHeadHash: owned.journalHead,
    expectedHeadMatches,
  };
}
