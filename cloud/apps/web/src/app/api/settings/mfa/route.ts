import { appendAuditEvent } from "@ghost/core/audit-log";
import { mfaKeyConfigured, openMfaSecret, sealMfaSecret } from "@ghost/core/crypto/mfa-secret";
import {
  generateRecoveryCodes,
  generateTotpSecret,
  hashRecoveryCode,
  totpUri,
  verifyTotp,
} from "@ghost/core/mfa";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";

/**
 * Two-factor enrolment.
 *
 * Three steps, deliberately:
 *
 *   GET    — status, so the UI never guesses.
 *   POST   — begin: mint a secret, store it sealed, return the otpauth URI.
 *            2FA is **not** active yet; `mfaEnabledAt` stays null.
 *   PUT    — confirm: verify a code from the app, then activate and issue
 *            recovery codes (shown once).
 *   DELETE — disable, and only on proof of a current code.
 *
 * The split between POST and PUT is the important part. Activating on POST
 * would lock out anyone who opened the enrolment screen, never scanned the
 * QR, and closed the tab — they would hold no working authenticator and be
 * unable to sign in. Nothing is enforced until a code round-trips.
 */

export async function GET(): Promise<Response> {
  const session = await auth();
  if (!session?.user?.id) return Response.json({ error: "unauthorized" }, { status: 401 });

  const user = await prisma.user.findUnique({
    where: { id: session.user.id },
    select: { mfaEnabledAt: true, mfaSecret: true },
  });
  const remaining = await prisma.mfaRecoveryCode.count({
    where: { userId: session.user.id, usedAt: null },
  });

  return Response.json({
    available: mfaKeyConfigured(),
    enabled: Boolean(user?.mfaEnabledAt),
    enrolling: Boolean(user?.mfaSecret) && !user?.mfaEnabledAt,
    recoveryCodesRemaining: remaining,
  });
}

export async function POST(): Promise<Response> {
  const session = await auth();
  if (!session?.user?.id || !session.user.email) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  if (!mfaKeyConfigured()) {
    return Response.json(
      { error: "two-factor is not configured on this deployment (GHOST_MFA_KEY is unset)" },
      { status: 503 },
    );
  }

  const user = await prisma.user.findUnique({
    where: { id: session.user.id },
    select: { mfaEnabledAt: true },
  });
  if (user?.mfaEnabledAt) {
    // Re-enrolling would silently invalidate the working authenticator.
    return Response.json({ error: "two-factor is already enabled" }, { status: 409 });
  }

  const secret = generateTotpSecret();
  await prisma.user.update({
    where: { id: session.user.id },
    data: { mfaSecret: sealMfaSecret(secret, session.user.id) },
  });

  return Response.json({
    secret,
    uri: totpUri({ secret, account: session.user.email, issuer: "Ghost" }),
  });
}

export async function PUT(req: Request): Promise<Response> {
  const session = await auth();
  if (!session?.user?.id) return Response.json({ error: "unauthorized" }, { status: 401 });
  const userId = session.user.id;

  const body = (await req.json().catch(() => null)) as { code?: unknown } | null;
  const code = typeof body?.code === "string" ? body.code : "";

  const user = await prisma.user.findUnique({
    where: { id: userId },
    select: { mfaSecret: true, mfaEnabledAt: true },
  });
  if (!user?.mfaSecret) {
    return Response.json({ error: "start enrolment first" }, { status: 409 });
  }
  if (user.mfaEnabledAt) {
    return Response.json({ error: "two-factor is already enabled" }, { status: 409 });
  }

  let secret: string;
  try {
    secret = openMfaSecret(user.mfaSecret, userId);
  } catch {
    return Response.json({ error: "stored secret could not be read; restart enrolment" }, { status: 500 });
  }
  if (!verifyTotp(secret, code)) {
    return Response.json({ error: "that code is not valid" }, { status: 400 });
  }

  const codes = generateRecoveryCodes();

  await prisma.$transaction(async (tx) => {
    await tx.user.update({ where: { id: userId }, data: { mfaEnabledAt: new Date() } });
    // Replace wholesale: enabling fresh must not leave codes from a previous
    // enrolment usable.
    await tx.mfaRecoveryCode.deleteMany({ where: { userId } });
    await tx.mfaRecoveryCode.createMany({
      data: codes.map((c) => ({ userId, codeHash: hashRecoveryCode(c) })),
    });
    if (session.user.orgId) {
      await appendAuditEvent(
        session.user.orgId,
        userId,
        { action: "user.mfa_enabled", entityType: "User", entityId: userId },
        tx,
      );
    }
  });

  // Shown exactly once. Only digests are stored.
  return Response.json({ recoveryCodes: codes });
}

export async function DELETE(req: Request): Promise<Response> {
  const session = await auth();
  if (!session?.user?.id) return Response.json({ error: "unauthorized" }, { status: 401 });
  const userId = session.user.id;

  const body = (await req.json().catch(() => null)) as { code?: unknown } | null;
  const code = typeof body?.code === "string" ? body.code : "";

  const user = await prisma.user.findUnique({
    where: { id: userId },
    select: { mfaSecret: true, mfaEnabledAt: true },
  });
  if (!user?.mfaEnabledAt || !user.mfaSecret) {
    return Response.json({ error: "two-factor is not enabled" }, { status: 409 });
  }

  // Turning off a security control requires the same proof as using it —
  // otherwise a stolen session alone is enough to strip the account's second
  // factor and keep it stripped.
  let secret: string;
  try {
    secret = openMfaSecret(user.mfaSecret, userId);
  } catch {
    return Response.json({ error: "stored secret could not be read" }, { status: 500 });
  }
  if (!verifyTotp(secret, code)) {
    return Response.json({ error: "that code is not valid" }, { status: 400 });
  }

  await prisma.$transaction(async (tx) => {
    await tx.user.update({
      where: { id: userId },
      data: { mfaSecret: null, mfaEnabledAt: null },
    });
    await tx.mfaRecoveryCode.deleteMany({ where: { userId } });
    if (session.user.orgId) {
      await appendAuditEvent(
        session.user.orgId,
        userId,
        { action: "user.mfa_disabled", entityType: "User", entityId: userId },
        tx,
      );
    }
  });

  return Response.json({ ok: true });
}
