import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { Card, CardBody } from "@/components/ui/card";
import { verifyAuditChain } from "@ghost/core/audit";
import { auditPayloadFromRow } from "@ghost/core/audit-log";

export const dynamic = "force-dynamic";

/**
 * The organization's tamper-evident ledger, and whether it still verifies.
 *
 * The hash chain existed from Phase 1 but nothing ever checked it — a log
 * nobody can verify is just a log. This page answers the question a customer's
 * auditor actually asks: has anything in this history been altered since it was
 * written?
 */
export default async function AuditPage() {
  const session = await auth();
  const orgId = session?.user.orgId;

  // Verification must walk the chain from its real first event. Verifying a
  // truncated window would report every healthy chain past the page size as
  // "broken at #0", because the oldest row in that window has a non-null
  // `prevHash` while the walk expects the first link's predecessor to be null —
  // a false alarm in the one feature whose entire job is trustworthy
  // verification.
  const [events, allForChain] = orgId
    ? await Promise.all([
        prisma.auditEvent.findMany({ where: { orgId }, orderBy: { seq: "desc" }, take: 200 }),
        prisma.auditEvent.findMany({
          where: { orgId },
          orderBy: { seq: "asc" },
          select: { action: true, entityType: true, entityId: true, metadata: true, prevHash: true, hash: true },
        }),
      ])
    : [[], []];

  const chain = verifyAuditChain(
    allForChain.map((e) => ({
      prevHash: e.prevHash,
      hash: e.hash,
      payload: auditPayloadFromRow(e),
    })),
  );

  return (
    <div className="mx-auto max-w-4xl space-y-6">
      <div>
        <h1 className="text-xl font-semibold">Audit</h1>
        <p className="mt-1 text-sm text-[var(--color-muted)]">
          Every run, approval, and mutation, hash-chained so alteration is detectable.
        </p>
      </div>

      <Card>
        <CardBody className="flex items-center justify-between">
          <div className="text-sm">
            <span className="font-medium">Chain integrity</span>
            <p className="mt-1 text-xs text-[var(--color-muted)]">
              {allForChain.length === 0
                ? "No events recorded yet."
                : `Full chain verified — all ${allForChain.length} events${
                    allForChain.length > events.length ? `, showing the most recent ${events.length}` : ""
                  }.`}
            </p>
          </div>
          <span
            className={`text-sm font-medium ${
              chain.intact ? "text-[var(--color-success)]" : "text-[var(--color-danger)]"
            }`}
          >
            {allForChain.length === 0
              ? "—"
              : chain.intact
                ? "Intact"
                : `Broken at #${(chain.firstBreakIndex ?? 0) + 1}`}
          </span>
        </CardBody>
      </Card>

      {events.length === 0 ? (
        <Card>
          <CardBody className="py-12 text-center">
            <p className="text-sm font-medium">Nothing to audit yet</p>
            <p className="mx-auto mt-1 max-w-md text-sm text-[var(--color-muted)]">
              Run a workflow and its every step, approval, and outcome will appear here.
            </p>
          </CardBody>
        </Card>
      ) : (
        <div className="space-y-1">
          {events.map((e) => (
            <Card key={e.id}>
              <CardBody className="flex items-baseline gap-4 py-2.5">
                <span className="w-12 shrink-0 font-mono text-xs text-[var(--color-muted)]">
                  #{e.seq}
                </span>
                <span className="w-56 shrink-0 text-sm font-medium">{e.action}</span>
                <span className="min-w-0 flex-1 truncate font-mono text-xs text-[var(--color-muted)]">
                  {e.entityType}
                  {e.entityId ? ` ${e.entityId}` : ""}
                </span>
                <span className="shrink-0 text-xs text-[var(--color-muted)]">
                  {e.createdAt.toLocaleString()}
                </span>
              </CardBody>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
