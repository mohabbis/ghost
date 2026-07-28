import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";

export const dynamic = "force-dynamic";

export default async function SettingsPage() {
  const session = await auth();
  const orgId = session?.user.orgId;

  const org = orgId
    ? await prisma.organization.findUnique({
        where: { id: orgId },
        include: {
          memberships: { include: { user: true }, orderBy: { createdAt: "asc" } },
        },
      })
    : null;

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <div>
        <h1 className="text-xl font-semibold">Settings</h1>
        <p className="mt-1 text-sm text-[var(--color-muted)]">Organization and members.</p>
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

      <Card>
        <CardHeader>
          <CardTitle>Members</CardTitle>
        </CardHeader>
        <CardBody className="space-y-2">
          {(org?.memberships ?? []).map((m) => (
            <div key={m.id} className="flex items-center justify-between text-sm">
              <span>{m.user.email ?? m.user.name ?? m.userId}</span>
              <span className="text-xs text-[var(--color-muted)]">{m.role}</span>
            </div>
          ))}
        </CardBody>
      </Card>
    </div>
  );
}
