import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { Card, CardBody } from "@/components/ui/card";

export const dynamic = "force-dynamic";

/**
 * The organization's tamper-evident ledger, and whether its expected head
 * still matches.
 *
 * Full mid-chain re-hashing of unbounded history is an API concern
 * (`GET /api/audit/verify?mode=full`) and is capped. This page used to load
 * the entire chain on every render (P1-1) — the integrity badge now uses the
 * cheap expected-head check, which still catches suffix deletion.
 */
export default async function AuditPage() {
  const session = await auth();
  const orgId = session?.user.orgId;

  const [events, orgRow, count, tail] = orgId
    ? await Promise.all([
        prisma.auditEvent.findMany({ where: { orgId }, orderBy: { seq: "desc" }, take: 200 }),
        prisma.organization.findUniqueOrThrow({
          where: { id: orgId },
          select: { auditChainHead: true },
        }),
        prisma.auditEvent.count({ where: { orgId } }),
        prisma.auditEvent.findFirst({
          where: { orgId },
          orderBy: { seq: "desc" },
          select: { hash: true },
        }),
      ])
    : [[], null, 0, null];

  const headMatches = (tail?.hash ?? null) === (orgRow?.auditChainHead ?? null);

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
              {count === 0
                ? "No events recorded yet."
                : `Expected head check across ${count} events${
                    count > events.length ? `, showing the most recent ${events.length}` : ""
                  }. Full mid-chain verify: GET /api/audit/verify?mode=full.`}
            </p>
          </div>
          <span
            className={`text-sm font-medium ${
              headMatches ? "text-[var(--color-success)]" : "text-[var(--color-danger)]"
            }`}
          >
            {count === 0 ? "—" : headMatches ? "Intact" : "Head mismatch"}
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
