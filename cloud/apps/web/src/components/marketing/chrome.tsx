import Link from "next/link";
import { Code2 } from "lucide-react";
import { SOURCE_URL } from "@/lib/source-url";

/**
 * Nav and footer for the signed-out surfaces (`/` and `/demo`).
 *
 * `signedIn` decides the primary call to action. A signed-in visitor landing on
 * `/` should not be told to "Sign in" — but they are also not force-redirected
 * into the app the way `/` used to redirect everyone, because a public page
 * that only exists for logged-out users is a page the owner can never look at.
 */

export function MarketingNav({ signedIn }: { signedIn: boolean }) {
  return (
    <header className="sticky top-0 z-20 border-b border-[var(--color-border)] bg-[var(--color-bg)]/85 backdrop-blur-md">
      <div className="mx-auto flex h-14 max-w-6xl items-center gap-6 px-5">
        <Link href="/" className="flex items-center gap-2 font-semibold">
          <span className="grid h-6 w-6 place-items-center rounded-md bg-[var(--color-accent)] text-xs font-bold text-[var(--color-accent-fg)]">
            G
          </span>
          Ghost
        </Link>

        <nav className="hidden items-center gap-5 text-sm text-[var(--color-muted)] md:flex">
          {/* `Link`, not `<a>`: these point at in-app routes (`/` plus a
              fragment), and from /demo they are real navigations. */}
          <Link className="transition-colors hover:text-[var(--color-fg)]" href="/#pipeline">
            How it works
          </Link>
          <Link className="transition-colors hover:text-[var(--color-fg)]" href="/#trust">
            Approvals
          </Link>
          <Link className="transition-colors hover:text-[var(--color-fg)]" href="/#status">
            What&rsquo;s built
          </Link>
          <Link className="transition-colors hover:text-[var(--color-fg)]" href="/demo">
            Walkthrough
          </Link>
        </nav>

        <div className="ml-auto flex items-center gap-2">
          <a
            href={SOURCE_URL}
            target="_blank"
            rel="noreferrer"
            className="hidden h-8 items-center gap-1.5 rounded-lg border border-[var(--color-border)] px-3 text-xs font-medium transition-colors hover:bg-[var(--color-surface)] sm:inline-flex"
          >
            <Code2 className="h-3.5 w-3.5" />
            Source
          </a>
          <Link
            href={signedIn ? "/dashboard" : "/signin"}
            className="inline-flex h-8 items-center rounded-lg bg-[var(--color-accent)] px-3 text-xs font-medium text-[var(--color-accent-fg)] transition-opacity hover:opacity-90"
          >
            {signedIn ? "Open dashboard" : "Sign in"}
          </Link>
        </div>
      </div>
    </header>
  );
}

export function MarketingFooter() {
  return (
    <footer className="border-t border-[var(--color-border)] px-5 py-10">
      <div className="mx-auto flex max-w-6xl flex-col gap-4 text-xs text-[var(--color-muted)] sm:flex-row sm:items-center sm:justify-between">
        <p>
          Ghost repeats computer tasks in a real browser, and stops for your approval before
          anything it can&rsquo;t undo.
        </p>
        {/*
          AGPL-3.0 section 13 reaches everyone who interacts with this instance
          over a network, including visitors who never sign in. The authenticated
          sidebar carries the same link; this is the one that covers them.
        */}
        <a
          href={SOURCE_URL}
          target="_blank"
          rel="noreferrer"
          className="inline-flex items-center gap-1.5 transition-colors hover:text-[var(--color-fg)]"
        >
          <Code2 className="h-3.5 w-3.5" />
          AGPL-3.0 · Source
        </a>
      </div>
    </footer>
  );
}
