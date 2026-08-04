import { randomUUID } from "node:crypto";
import NextAuth, { type NextAuthConfig } from "next-auth";
import Credentials from "next-auth/providers/credentials";
import GitHub from "next-auth/providers/github";
import { assertAuthSecretUsable, devSignInAllowed, sessionMaxAgeSeconds } from "@/lib/auth-env";
import { ensureUserOrg } from "@/lib/org";

/**
 * Auth.js (NextAuth v5) configuration.
 *
 * JWT sessions so the local dev-credentials provider works without a mail
 * server. On sign-in we persist the user + bootstrap an Organization
 * (`ensureUserOrg`) and stamp `userId`/`orgId` into the token, so every request
 * is scoped to a tenant.
 *
 * GitHub OAuth is enabled only when its env vars are present. Locally, the
 * "Dev sign-in" provider accepts any email.
 */

// Before anything else: in production, refuse to start on a session secret
// that cannot protect a session. See lib/auth-env.ts.
assertAuthSecretUsable();

const providers: NextAuthConfig["providers"] = [];

if (process.env.AUTH_GITHUB_ID && process.env.AUTH_GITHUB_SECRET) {
  providers.push(GitHub);
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
