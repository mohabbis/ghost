import { auth } from "@/auth";
import { NextResponse } from "next/server";

/**
 * Protect the authenticated app. Anything under the matched paths requires a
 * session; unauthenticated requests are redirected to /signin.
 */
export default auth((req) => {
  if (!req.auth) {
    const url = new URL("/signin", req.nextUrl.origin);
    url.searchParams.set("from", req.nextUrl.pathname);
    return NextResponse.redirect(url);
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
  ],
};
