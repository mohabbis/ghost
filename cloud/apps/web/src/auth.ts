import NextAuth, { type NextAuthConfig } from "next-auth";
import Credentials from "next-auth/providers/credentials";
import GitHub from "next-auth/providers/github";
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

const providers: NextAuthConfig["providers"] = [];

if (process.env.AUTH_GITHUB_ID && process.env.AUTH_GITHUB_SECRET) {
  providers.push(GitHub);
}

// Dev-only email sign-in. Never enabled in production.
if (process.env.NODE_ENV !== "production") {
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
  session: { strategy: "jwt" },
  pages: { signIn: "/signin" },
  callbacks: {
    async jwt({ token, user }) {
      // Runs with `user` only on initial sign-in.
      if (user?.email) {
        const { userId, orgId } = await ensureUserOrg(user.email, user.name, user.image);
        token.userId = userId;
        token.orgId = orgId;
      }
      return token;
    },
    session({ session, token }) {
      if (typeof token.userId === "string") session.user.id = token.userId;
      if (typeof token.orgId === "string") session.user.orgId = token.orgId;
      return session;
    },
  },
};

export const { handlers, auth, signIn, signOut } = NextAuth(authConfig);
