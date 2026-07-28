import { redirect } from "next/navigation";
import { auth, signIn } from "@/auth";
import { Button } from "@/components/ui/button";
import { Card, CardBody } from "@/components/ui/card";

const githubEnabled = Boolean(process.env.AUTH_GITHUB_ID && process.env.AUTH_GITHUB_SECRET);
const devEnabled = process.env.NODE_ENV !== "production";

export default async function SignInPage({
  searchParams,
}: {
  searchParams: Promise<{ from?: string }>;
}) {
  if (await auth()) redirect("/dashboard");
  const { from } = await searchParams;
  const redirectTo = from ?? "/dashboard";

  return (
    <main className="flex min-h-screen items-center justify-center px-4">
      <Card className="w-full max-w-sm">
        <CardBody className="space-y-6">
          <div className="space-y-1">
            <div className="text-lg font-semibold">Ghost</div>
            <p className="text-sm text-[var(--color-muted)]">
              Sign in to your workspace.
            </p>
          </div>

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
    </main>
  );
}
