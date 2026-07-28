/** Base URL the worker uses to reach app-served pages (e.g. the demo fixture). */
export function appUrl(): string {
  return (
    process.env.APP_URL ??
    process.env.AUTH_URL ??
    process.env.NEXTAUTH_URL ??
    "http://localhost:3000"
  ).replace(/\/$/, "");
}
