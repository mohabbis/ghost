import { prisma } from "./db";

/** Turn an email/name into a readable org name + unique-ish slug. */
function deriveOrg(email: string, name?: string | null): { name: string; slug: string } {
  const handle = (name?.trim() || email.split("@")[0] || "workspace").trim();
  const base =
    handle
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "workspace";
  const suffix = Math.random().toString(36).slice(2, 7);
  return { name: `${handle}'s workspace`, slug: `${base}-${suffix}` };
}

/**
 * Ensure a signed-in user has a persisted `User`, an `Organization`, and a
 * `Membership`. Idempotent — safe to call on every sign-in. Returns the ids the
 * session needs. This is what makes the app multi-tenant from the first login.
 */
export async function ensureUserOrg(
  email: string,
  name?: string | null,
  image?: string | null,
): Promise<{ userId: string; orgId: string }> {
  const user = await prisma.user.upsert({
    where: { email },
    update: { name: name ?? undefined, image: image ?? undefined },
    create: { email, name: name ?? undefined, image: image ?? undefined },
  });

  const existing = await prisma.membership.findFirst({
    where: { userId: user.id },
    orderBy: { createdAt: "asc" },
  });
  if (existing) return { userId: user.id, orgId: existing.orgId };

  const { name: orgName, slug } = deriveOrg(email, name);
  const org = await prisma.organization.create({
    data: {
      name: orgName,
      slug,
      memberships: { create: { userId: user.id, role: "OWNER" } },
    },
  });
  return { userId: user.id, orgId: org.id };
}
