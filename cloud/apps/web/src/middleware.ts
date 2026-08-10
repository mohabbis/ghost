import NextAuth from "next-auth";
import { NextResponse } from "next/server";
import { edgeAuthConfig } from "@/auth.config";
import { MFA_COOKIE_NAME, verifyMfaCookie } from "@/lib/mfa-cookie";

/**
 * Protect the authenticated app. Anything under the matched paths requires a
 * session; unauthenticated requests are redirected to /signin.
 *
 * Built from `edgeAuthConfig` rather than `@/auth`: this runs in the Edge
 * runtime, and importing the full config pulls Prisma and `node:crypto` in
 * with it, which do not exist there. See `auth.config.ts`.
 */
const { auth } = NextAuth(edgeAuthConfig);
/** An API caller wants a status code, not an HTML login page. */
function isApiPath(pathname: string): boolean {
  return pathname.startsWith("/api/");
}

function challenge(req: { nextUrl: URL; cookies: unknown }): NextResponse {
  if (isApiPath(req.nextUrl.pathname)) {
    return NextResponse.json({ error: "two-factor verification required" }, { status: 403 });
  }
  const url = new URL("/mfa", req.nextUrl.origin);
  url.searchParams.set("from", req.nextUrl.pathname);
  return NextResponse.redirect(url);
}

export default auth(async (req) => {
  if (!req.auth) {
    if (isApiPath(req.nextUrl.pathname)) {
      return NextResponse.json({ error: "unauthorized" }, { status: 401 });
    }
    const url = new URL("/signin", req.nextUrl.origin);
    url.searchParams.set("from", req.nextUrl.pathname);
    return NextResponse.redirect(url);
  }

  // Second factor. Enforced here rather than in a layout precisely so it also
  // covers `/api/*`: gating only the pages would leave every bit of the same
  // data reachable by calling the API directly with the session cookie, which
  // defeats the point of having a challenge at all.
  const user = req.auth.user;
  if (user?.mfaEnabled && user.id) {
    const verified = await verifyMfaCookie(
      req.cookies.get(MFA_COOKIE_NAME)?.value,
      user.id,
      user.sid,
    );
    if (!verified) return challenge(req);
  }

  return NextResponse.next();
});

/**
 * One entry per route group under `src/app/(app)`.
 *
 * This has to be an inline literal: Next.js statically analyses
 * `config.matcher` at build time and rejects an imported identifier
 * ("Unknown identifier ... at config.matcher"), so it cannot be lifted into a
 * shared module however much tidier that would be. `middleware.test.ts`
 * therefore reads this file and parses the list, rather than importing it.
 *
 * `(app)/layout.tsx` also redirects when there is no session, and that is what
 * actually stops an unauthenticated render — this matcher is the cheaper
 * outer gate, not the only one. `/audit` and `/recordings` both shipped
 * without an entry here; the test now fails when the two drift apart.
 */
export const config = {
  matcher: [
    "/audit/:path*",
    "/dashboard/:path*",
    "/recordings/:path*",
    "/runs/:path*",
    "/settings/:path*",
    "/workflows/:path*",
    // Session-authenticated API surface, so the second-factor gate above
    // applies to it too. Three exclusions, each load-bearing:
    //   auth  — Auth.js's own endpoints; gating them prevents signing in.
    //   agent — bearer-credential authenticated only (no session). A session
    //           gate here would reject every legitimate MCP/agent client;
    //           `resolveAgentPrincipal` refuses cookies and requires a minted
    //           API key — see lib/agent-auth.ts.
    //   mfa   — how the challenge is answered; gating it deadlocks the user.
    "/api/((?!auth|agent|mfa).*)",
  ],
};
