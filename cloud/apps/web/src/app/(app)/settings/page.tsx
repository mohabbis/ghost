import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { isOrgAdmin } from "@ghost/core/roles";
import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";
import { Members } from "@/components/settings/members";
import { AgentCredentials } from "@/components/settings/agent-credentials";

export const dynamic = "force-dynamic";

export default async function SettingsPage() {
  const session = await auth();
  const orgId = session?.user.orgId;

  const org = orgId
    ? await prisma.organization.findUnique({
        where: { id: orgId },
        include: {
          memberships: {
            include: { user: true },
            orderBy: { createdAt: "asc" },
          },
        },
      })
    : null;

  const currentMembership = org?.memberships.find((m) => m.userId === session?.user.id);
  const isAdmin = currentMembership ? isOrgAdmin(currentMembership.role) : false;

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <div>
        <h1 className="text-xl font-semibold">Settings</h1>
        <p className="mt-1 text-sm text-[var(--color-muted)]">
          Organization and members.
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Organization</CardTitle>
        </CardHeader>
        <CardBody className="space-y-1 text-sm">
          <div className="flex justify-between">
            <span className="text-[var(--color-muted)]">Name</span>
            <span>{org?.name ?? "—"}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-[var(--color-muted)]">Slug</span>
            <span className="font-mono text-xs">{org?.slug ?? "—"}</span>
          </div>
        </CardBody>
      </Card>

      {/* Replaced the read-only member list: roles are now editable and
          invitations are issued here. */}
      <Members isOrgAdmin={isAdmin} />

      <AgentCredentials isOrgAdmin={isAdmin} />
    </div>
  );
}
