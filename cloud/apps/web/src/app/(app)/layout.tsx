import { redirect } from "next/navigation";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { AppSidebar } from "@/components/app-sidebar";
import { SignOutButton } from "@/components/sign-out-button";

export const dynamic = "force-dynamic";

export default async function AppLayout({ children }: { children: React.ReactNode }) {
  const session = await auth();
  if (!session?.user) redirect("/signin");

  const org = session.user.orgId
    ? await prisma.organization.findUnique({ where: { id: session.user.orgId } })
    : null;

  return (
    <div className="flex min-h-screen">
      <AppSidebar orgName={org?.name ?? "Workspace"} />
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 items-center justify-between border-b border-[var(--color-border)] px-6">
          <div className="text-sm text-[var(--color-muted)]">{session.user.email}</div>
          <SignOutButton />
        </header>
        <main className="flex-1 p-6">{children}</main>
      </div>
    </div>
  );
}
