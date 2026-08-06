import Link from "next/link";
import { redirect } from "next/navigation";
import { auth } from "@/auth";
import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";
import { SOURCE_URL } from "@/lib/source-url";

const CAPABILITIES = [
  "Record a browser workflow",
  "Convert it into editable steps",
  "Replay across browser / API actions",
  "Approve sensitive actions before they execute",
  "Verify the outcome and log every run",
];

/**
 * Public landing page. Signed-in visitors go straight to their dashboard;
 * everyone else sees what Ghost does before being asked to sign in — the
 * previous version of this page redirected unconditionally to /signin,
 * which meant the site's only public face was a bare auth wall with no
 * context for what it was gating.
 */
export default async function Home() {
  const session = await auth();
  if (session) redirect("/dashboard");

  return (
    <main className="flex min-h-screen flex-col items-center justify-center px-4 py-16">
      <div className="w-full max-w-lg space-y-6">
        <div className="space-y-2 text-center">
          <div className="text-lg font-semibold">Ghost</div>
          <p className="text-sm text-[var(--color-muted)]">
            Teach Ghost a workflow once — it executes it reliably, pauses for approval on
            sensitive steps, verifies the outcome, and logs what changed.
          </p>
        </div>

        <Card>
          <CardHeader>
            <CardTitle>What Ghost does</CardTitle>
          </CardHeader>
          <CardBody className="space-y-2">
            {CAPABILITIES.map((label) => (
              <div key={label} className="text-sm">
                {label}
              </div>
            ))}
          </CardBody>
        </Card>

        <Link
          href="/signin"
          className="flex h-10 w-full items-center justify-center rounded-lg bg-[var(--color-accent)] text-sm font-medium text-[var(--color-accent-fg)] transition-colors hover:opacity-90"
        >
          Sign in
        </Link>
      </div>

      {/*
        AGPL-3.0 section 13 covers everyone interacting with this instance over
        the network, including a visitor who never signs in.
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
