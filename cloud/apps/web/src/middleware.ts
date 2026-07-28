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

export const config = {
  matcher: ["/dashboard/:path*", "/workflows/:path*", "/runs/:path*", "/settings/:path*"],
};
