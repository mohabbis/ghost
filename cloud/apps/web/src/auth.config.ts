import type { NextAuthConfig } from "next-auth";

/**
 * The slice of the Auth.js config that is safe to bundle for the Edge runtime.
 *
 * `middleware.ts` runs on the Edge, and importing the full `auth.ts` drags its
 * entire import graph in with it — Prisma, the audit hash chain, invitation
 * token hashing, and therefore `node:crypto`, which does not exist there. The
 * build fails outright ("Module not found: node:crypto").
 *
 * Splitting is the documented Auth.js v5 approach, and it works because the
 * middleware only needs to answer *"is there a valid session token?"*. With
 * the JWT strategy that is a signature check against AUTH_SECRET — no
 * database, no providers, none of the callbacks that mint the token. Those
 * live in `auth.ts`, which runs in the Node runtime where its dependencies
 * are real.
 *
 * Keep this file free of anything that touches the database, `node:*`, or a
 * provider SDK. If middleware ever needs a claim, add it to the token in
 * `auth.ts`'s `jwt` callback and read it here.
 */
export const edgeAuthConfig: NextAuthConfig = {
  providers: [],
  session: { strategy: "jwt" },
  pages: { signIn: "/signin" },
};
