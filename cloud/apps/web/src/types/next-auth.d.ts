import type { DefaultSession } from "next-auth";

declare module "next-auth" {
  interface Session {
    user: {
      id: string;
      orgId: string;
      /** Account requires a second factor. Whether one was *passed* this
       *  session lives in the signed MFA cookie, not here. */
      mfaEnabled?: boolean;
    sid?: string;
      /** Per-sign-in id the MFA cookie is bound to. */
      sid?: string;
    } & DefaultSession["user"];
  }
}

declare module "next-auth/jwt" {
  interface JWT {
    userId?: string;
    orgId?: string;
    mfaEnabled?: boolean;
    sid?: string;
  }
}
