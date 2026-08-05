import { redirect } from "next/navigation";
import { auth, signIn } from "@/auth";
import { Button } from "@/components/ui/button";
import { Card, CardBody } from "@/components/ui/card";
import { SOURCE_URL } from "@/lib/source-url";

const githubEnabled = Boolean(process.env.AUTH_GITHUB_ID && process.env.AUTH_GITHUB_SECRET);
const googleEnabled = Boolean(process.env.AUTH_GOOGLE_ID && process.env.AUTH_GOOGLE_SECRET);
const devEnabled = process.env.NODE_ENV !== "production";
// All three false means production with no OAuth provider configured: no
// form below has anything to render, and a card with a title and no buttons
// looks like a bug rather than a missing deploy step. Whoever hits this is
// more likely to be the person standing up the deployment than an end user,
// so name the exact fix rather than failing silently. See docs/DEPLOY.md's
// "sign-in trap".
const misconfigured = !githubEnabled && !googleEnabled && !devEnabled;

export default async function SignInPage({
  searchParams,
}: {
  searchParams: Promise<{ from?: string }>;
}) {
  if (await auth()) redirect("/dashboard");
  const { from } = await searchParams;
  const redirectTo = from ?? "/dashboard";

  return (
    <main className="flex min-h-screen flex-col items-center justify-center px-4">
      <Card className="w-full max-w-sm">
        <CardBody className="space-y-6">
          <div className="space-y-1">
            <div className="text-lg font-semibold">Ghost</div>
            <p className="text-sm text-[var(--color-muted)]">
              Sign in to your workspace.
            </p>
          </div>

          {misconfigured && (
            <div
              role="alert"
              className="space-y-2 rounded-lg border border-[var(--color-danger)] bg-[var(--color-danger)]/10 p-3 text-sm"
            >
              <p className="font-medium text-[var(--color-danger)]">
                No sign-in method is configured
              </p>
              <p className="text-[var(--color-muted)]">
                This deployment has <code>NODE_ENV=production</code> and no OAuth app
                configured, so there is no way to sign in. Set either{" "}
                <code>AUTH_GITHUB_ID</code>/<code>AUTH_GITHUB_SECRET</code> or{" "}
                <code>AUTH_GOOGLE_ID</code>/<code>AUTH_GOOGLE_SECRET</code>, with the
                app&apos;s callback at{" "}
                <code>https://&lt;this-domain&gt;/api/auth/callback/&lt;github|google&gt;</code>.
                See docs/DEPLOY.md.
              </p>
            </div>
          )}

          {githubEnabled && (
            <form
              action={async () => {
                "use server";
                await signIn("github", { redirectTo });
              }}
            >
              <Button type="submit" variant="secondary" className="w-full">
                Continue with GitHub
              </Button>
            </form>
          )}

          {googleEnabled && (
            <form
              action={async () => {
                "use server";
                await signIn("google", { redirectTo });
              }}
            >
              <Button type="submit" variant="secondary" className="w-full">
                Continue with Google
              </Button>
            </form>
          )}

          {devEnabled && (
            <form
              action={async (formData: FormData) => {
                "use server";
                await signIn("dev", {
                  email: String(formData.get("email") ?? ""),
                  redirectTo,
                });
              }}
              className="space-y-3"
            >
              <label className="block space-y-1">
                <span className="text-xs font-medium text-[var(--color-muted)]">
                  Dev sign-in (any email)
                </span>
                <input
                  name="email"
                  type="email"
                  required
                  placeholder="you@example.com"
                  className="h-10 w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 text-sm outline-none focus-visible:border-[var(--color-accent)]"
                />
              </label>
              <Button type="submit" className="w-full">
                Sign in
              </Button>
            </form>
          )}
        </CardBody>
      </Card>

      {/*
        Also offered here, not only behind the sidebar: AGPL-3.0 section 13 covers
        everyone interacting with this instance over the network, and someone who
        never signs in never sees the authenticated chrome.
      */}
      <a
        href={SOURCE_URL}
        target="_blank"
        rel="noreferrer"
        className="mt-6 text-xs text-[var(--color-muted)] hover:text-[var(--color-fg)]"
      >
        AGPL-3.0 · Source
      </a>
    </main>
  );
}
