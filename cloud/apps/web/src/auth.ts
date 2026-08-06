import { randomUUID } from "node:crypto";
import { PrismaAdapter } from "@auth/prisma-adapter";
import NextAuth, { type NextAuthConfig } from "next-auth";
import Credentials from "next-auth/providers/credentials";
import GitHub from "next-auth/providers/github";
import Google from "next-auth/providers/google";
import Resend from "next-auth/providers/resend";
import { assertAuthSecretUsable, devSignInAllowed, sessionMaxAgeSeconds } from "@/lib/auth-env";
import { prisma } from "@/lib/db";
import { ensureUserOrg } from "@/lib/org";

/**
 * Auth.js (NextAuth v5) configuration.
 *
 * JWT sessions so the local dev-credentials provider works without a mail
 * server. On sign-in we persist the user + bootstrap an Organization
 * (`ensureUserOrg`) and stamp `userId`/`orgId` into the token, so every request
 * is scoped to a tenant.
 *
 * GitHub, Google, and Resend (email magic link) are each enabled only when
 * their own env vars are present — a deployment can offer any combination
 * (falling back to the dev-only provider below). Locally, the "Dev sign-in"
 * provider accepts any email.
 */

// Before anything else: in production, refuse to start on a session secret
// that cannot protect a session. See lib/auth-env.ts.
assertAuthSecretUsable();

// The Email/magic-link provider is the only one that needs a database
// adapter (Auth.js stores the one-time token via it) — GitHub/Google/dev
// sign-in all work adapter-less today via `ensureUserOrg`'s manual upsert.
// Only wire the adapter in when Resend is actually configured, so a
// deployment without it is byte-for-byte the same as before this provider
// existed.
const resendEnabled = Boolean(process.env.RESEND_API_KEY && process.env.RESEND_EMAIL_DOMAIN);

const providers: NextAuthConfig["providers"] = [];

if (process.env.AUTH_GITHUB_ID && process.env.AUTH_GITHUB_SECRET) {
  providers.push(
    GitHub({
      // Once the adapter above is present, Auth.js refuses an OAuth sign-in
      // whose email already belongs to a different provider's account,
      // unless a provider opts in here. `ensureUserOrg` has always merged
      // accounts by email with no such check, so this keeps that existing
      // behavior instead of silently locking out a user who e.g. signed up
      // with GitHub and later tries Google. Inert (and unread) when the
      // adapter isn't wired.
      allowDangerousEmailAccountLinking: true,
    }),
  );
}

if (process.env.AUTH_GOOGLE_ID && process.env.AUTH_GOOGLE_SECRET) {
  providers.push(Google({ allowDangerousEmailAccountLinking: true }));
}

if (resendEnabled) {
  providers.push(
    Resend({
      apiKey: process.env.RESEND_API_KEY,
      from: `Ghost <noreply@${process.env.RESEND_EMAIL_DOMAIN}>`,
    }),
  );
}

// Passwordless "any email" sign-in for local development. `devSignInAllowed`
// requires production to be ruled out AND the instance to be loopback-only,
// so a deployment with NODE_ENV unset does not quietly expose it.
if (devSignInAllowed()) {
  providers.push(
    Credentials({
      id: "dev",
      name: "Dev sign-in",
      credentials: { email: { label: "Email", type: "email" } },
      authorize: (raw) => {
        const email = typeof raw?.email === "string" ? raw.email.trim() : "";
        if (!email || !email.includes("@")) return null;
        return { id: email, email, name: email.split("@")[0] };
      },
    }),
  );
}

export const authConfig: NextAuthConfig = {
  adapter: resendEnabled ? PrismaAdapter(prisma) : undefined,
  providers,
  session: { strategy: "jwt", maxAge: sessionMaxAgeSeconds() },
  pages: { signIn: "/signin" },
  callbacks: {
    async jwt({ token, user }) {
      // Runs with `user` only on initial sign-in.
      if (user?.email) {
        const { userId, orgId, mfaEnabled } = await ensureUserOrg(user.email, user.name, user.image);
        token.userId = userId;
        token.orgId = orgId;
        // Whether this account *requires* a second factor. Whether it has
        // *passed* one lives in a separate signed cookie, deliberately not
        // here — see lib/mfa-cookie.ts.
        token.mfaEnabled = mfaEnabled;
        // A fresh identifier for *this* sign-in. The MFA cookie is bound to
        // it, so signing out and back in mints a new one and the old cookie
        // stops satisfying the gate — otherwise a second factor proved once
        // would keep being accepted for every later sign-in within the
        // cookie's lifetime, which is exactly what MFA is supposed to prevent.
        token.sid = randomUUID();
      }
      return token;
    },
    session({ session, token }) {
      if (typeof token.userId === "string") session.user.id = token.userId;
      if (typeof token.orgId === "string") session.user.orgId = token.orgId;
      session.user.mfaEnabled = token.mfaEnabled === true;
      if (typeof token.sid === "string") session.user.sid = token.sid;
      return session;
    },
  },
};

export const { handlers, auth, signIn, signOut } = NextAuth(authConfig);
