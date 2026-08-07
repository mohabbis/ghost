import { signOut } from "@/auth";
import { Button } from "@/components/ui/button";
import { Card, CardBody } from "@/components/ui/card";

/**
 * Auth.js's own `/api/auth/signout` confirmation page renders its
 * unstyled built-in theme — the only unbranded surface in an otherwise
 * consistently dark-themed app. `pages.signOut` in `@/auth` points here
 * instead so a visitor never sees it, even though the real in-app
 * sign-out (`components/sign-out-button.tsx`) bypasses this page entirely
 * via the same server action.
 */
export default function SignOutPage() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center px-4">
      <Card className="w-full max-w-sm">
        <CardBody className="space-y-6">
          <div className="space-y-1">
            <div className="text-lg font-semibold">Ghost</div>
            <p className="text-sm text-[var(--color-muted)]">Sign out of your workspace.</p>
          </div>
          <form
            action={async () => {
              "use server";
              await signOut({ redirectTo: "/signin" });
            }}
          >
            <Button type="submit" variant="secondary" className="w-full">
              Sign out
            </Button>
          </form>
        </CardBody>
      </Card>
    </main>
  );
}
