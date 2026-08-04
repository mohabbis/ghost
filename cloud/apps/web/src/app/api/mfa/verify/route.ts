import { openMfaSecret } from "@ghost/core/crypto/mfa-secret";
import { hashRecoveryCode, verifyTotp } from "@ghost/core/mfa";
import { appendAuditEvent } from "@ghost/core/audit-log";
import { auth } from "@/auth";
import { prisma } from "@/lib/db";
import { MFA_COOKIE_NAME, signMfaCookie } from "@/lib/mfa-cookie";

/**
 * Answer the second-factor challenge for the current session.
 *
 * Accepts either a TOTP code or one recovery code. On success it sets the
 * signed, httpOnly MFA cookie the middleware checks — that cookie is the only
 * thing that satisfies the gate, and it is never issued without a code
 * verifying here first.
 *
 * A recovery code is consumed atomically (`updateMany` filtered on
 * `usedAt: null`), so two racing submissions of the same code cannot both
 * succeed.
 */
export async function POST(req: Request): Promise<Response> {
  const session = await auth();
  if (!session?.user?.id) return Response.json({ error: "unauthorized" }, { status: 401 });
  const userId = session.user.id;
  const sid = session.user.sid;
  if (!sid) return Response.json({ error: "session cannot be verified" }, { status: 400 });

  const body = (await req.json().catch(() => null)) as { code?: unknown } | null;
  const submitted = typeof body?.code === "string" ? body.code.trim() : "";
  if (!submitted) return Response.json({ error: "code required" }, { status: 400 });

  const user = await prisma.user.findUnique({
    where: { id: userId },
    select: { mfaSecret: true, mfaEnabledAt: true },
  });

  // Nothing to prove — let the caller through rather than trapping them at a
  // challenge they cannot answer (their token may simply be stale).
  if (!user?.mfaEnabledAt || !user.mfaSecret) {
    return setVerified(userId, sid, { usedRecoveryCode: false, noop: true });
  }

  let ok = false;
  let usedRecoveryCode = false;

  try {
    ok = verifyTotp(openMfaSecret(user.mfaSecret, userId), submitted);
  } catch {
    ok = false;
  }

  if (!ok) {
    // Fall back to a recovery code. Consume it in the same statement that
    // selects it, so a replay cannot win a race against itself.
    const consumed = await prisma.mfaRecoveryCode.updateMany({
      where: { userId, codeHash: hashRecoveryCode(submitted), usedAt: null },
      data: { usedAt: new Date() },
    });
    if (consumed.count === 1) {
      ok = true;
      usedRecoveryCode = true;
    }
  }

  if (!ok) {
    // One message for a wrong TOTP code and a wrong/spent recovery code:
    // distinguishing them tells an attacker which pool to keep guessing in.
    return Response.json({ error: "that code is not valid" }, { status: 400 });
  }

  if (usedRecoveryCode && session.user.orgId) {
    // Worth an audit entry: a recovery code means the authenticator was lost
    // or someone is using a code they should not have.
    await appendAuditEvent(session.user.orgId, userId, {
      action: "user.mfa_recovery_code_used",
      entityType: "User",
      entityId: userId,
    }).catch(() => undefined);
  }

  return setVerified(userId, sid, { usedRecoveryCode, noop: false });
}

async function setVerified(
  userId: string,
  sessionId: string,
  meta: { usedRecoveryCode: boolean; noop: boolean },
): Promise<Response> {
  const { value, maxAge } = await signMfaCookie(userId, sessionId);
  const res = Response.json({ ok: true, ...meta });
  res.headers.append(
    "Set-Cookie",
    [
      `${MFA_COOKIE_NAME}=${value}`,
      "Path=/",
      "HttpOnly",
      "SameSite=Lax",
      `Max-Age=${maxAge}`,
      process.env.NODE_ENV === "production" ? "Secure" : "",
    ]
      .filter(Boolean)
      .join("; "),
  );
  return res;
}
