import Link from "next/link";
import type { Metadata } from "next";
import { ArrowRight, Check, Minus } from "lucide-react";
import { auth } from "@/auth";
import { MarketingNav, MarketingFooter } from "@/components/marketing/chrome";
import { RunPreview } from "@/components/marketing/run-preview";
import { PIPELINE } from "@/lib/demo-narrative";

/**
 * The public front door.
 *
 * This route used to be five lines that `redirect()`ed every visitor to
 * `/signin`, which meant the only thing anyone could ever see of Ghost was a
 * login card on a dark background. A product nobody can look at reads as a
 * broken one. Everything here is public and static; the app itself stays
 * behind auth, unchanged.
 *
 * Copy rule for this file (CLAUDE.md rule 10, plus a note from the owner):
 * describe the mechanism, not the posture. Say "it stops before clicking
 * Submit", not "governed execution with deny-by-default policy". If a sentence
 * would survive being pasted onto a different product's homepage, it is not
 * specific enough to be here.
 */

export const metadata: Metadata = {
  title: "Ghost — it does the clicking, you approve what matters",
  description:
    "Show Ghost a task once. It repeats it in a real browser — opening pages, filling forms, moving data between systems — and stops for your approval before it submits, sends, pays, or deletes.",
};

/** Actions that always stop and wait. Deliberately the plainest possible words. */
const ALWAYS_STOPS = ["Submit", "Send", "Pay", "Delete", "Overwrite"];

const BUILT = [
  {
    title: "Runs steps in a real browser",
    body: "A server-side Chrome opens pages, fills fields, and clicks buttons — finding them by their visible label, so a redesigned page doesn't silently break the run.",
  },
  {
    title: "Stops for approval",
    body: "Anything that submits, sends, pays, or deletes halts the run. A person clicks Approve or Reject. Which actions count is a fixed rule in code — no model decides it.",
  },
  {
    title: "Checks its own work",
    body: "A step can assert what should be true afterwards. If the page doesn't say what it should, the run fails instead of reporting success.",
  },
  {
    title: "Screenshots every step",
    body: "You can see exactly what the browser was looking at when it did each thing, and what it looked like after.",
  },
  {
    title: "Keeps a log you can't quietly edit",
    body: "Each entry is hashed together with the one before it. Change or delete an old entry and the chain stops matching — the Audit page checks this and tells you where it broke.",
  },
  {
    title: "Survives crashes without redoing work",
    body: "Each run keeps a journal of what already finished. If a worker dies mid-run, the next one reads the journal and picks up after the last completed step — it won't re-submit an order that already went through.",
  },
  {
    title: "Takes work from other programs",
    body: "An HTTP and MCP surface lets an agent or script propose a run. It still can't skip the approval step — proposing and executing are separate.",
  },
];

const NEXT_UP = [
  {
    title: "Recording your screen",
    body: "The browser extension that captures a session exists; turning a raw capture into a clean step list is the piece still being built. Today you write or edit the steps directly.",
  },
  {
    title: "Direct connections to other apps",
    body: "Gmail, Salesforce, QuickBooks and the like, so Ghost can use their APIs instead of clicking through their websites. The database schema is in place; none are wired up yet.",
  },
];

export default async function Home() {
  const session = await auth();
  const signedIn = Boolean(session?.user);

  return (
    <>
      <MarketingNav signedIn={signedIn} />

      <main>
        {/* ---------------------------------------------------------------- */}
        {/* Hero                                                             */}
        {/* ---------------------------------------------------------------- */}
        <section className="relative overflow-hidden px-5 pt-16 pb-20 sm:pt-24">
          {/* Soft accent wash. Pointer-events-none so it never eats a click. */}
          <div
            aria-hidden="true"
            className="pointer-events-none absolute inset-x-0 top-0 -z-10 h-[420px] bg-[radial-gradient(60%_100%_at_50%_0%,var(--color-accent)/8%,transparent_75%)]"
          />
          <div className="mx-auto grid max-w-6xl items-center gap-12 lg:grid-cols-[1fr_1.05fr]">
            <div>
              {/* No hardcoded <br>: the column is narrower than the copy at
                  this size, so manual breaks stacked on top of the natural
                  wrap and stranded single words on their own lines. Let it
                  wrap and let `text-balance` even out the ragged edge. */}
              <h1 className="text-4xl leading-[1.1] font-semibold tracking-tight text-balance sm:text-[2.75rem] lg:text-5xl">
                Ghost does the clicking. You approve{" "}
                <span className="text-[var(--color-accent)]">
                  anything that can&rsquo;t be undone.
                </span>
              </h1>

              <p className="mt-6 max-w-xl text-base leading-relaxed text-[var(--color-muted)]">
                Show it a task once. It repeats it in a real browser — opening pages, filling
                forms, moving data between systems that don&rsquo;t talk to each other. Before it
                submits, sends, pays, or deletes, it stops and waits for you.
              </p>

              <div className="mt-8 flex flex-wrap items-center gap-3">
                <Link
                  href="/demo"
                  className="inline-flex h-10 items-center gap-2 rounded-lg bg-[var(--color-accent)] px-4 text-sm font-medium text-[var(--color-accent-fg)] transition-opacity hover:opacity-90"
                >
                  See it step by step
                  <ArrowRight className="h-4 w-4" />
                </Link>
                <Link
                  href={signedIn ? "/dashboard" : "/signin"}
                  className="inline-flex h-10 items-center rounded-lg border border-[var(--color-border)] px-4 text-sm font-medium transition-colors hover:bg-[var(--color-surface)]"
                >
                  {signedIn ? "Open dashboard" : "Sign in"}
                </Link>
              </div>

              <p className="mt-5 text-xs text-[var(--color-muted)]">
                Early stage and openly so — the execution engine works end to end. Screen
                recording is still being built. Nothing below claims otherwise.
              </p>
            </div>

            <RunPreview />
          </div>
        </section>

        {/* ---------------------------------------------------------------- */}
        {/* The concrete problem                                             */}
        {/* ---------------------------------------------------------------- */}
        <section className="border-y border-[var(--color-border)] bg-[var(--color-surface)]/50 px-5 py-16">
          <div className="mx-auto grid max-w-6xl gap-10 md:grid-cols-2">
            <div>
              <h2 className="text-xs font-medium tracking-widest text-[var(--color-muted)] uppercase">
                The work this replaces
              </h2>
              <p className="mt-4 text-lg leading-relaxed">
                A purchase order arrives as a PDF. Someone opens it, retypes twelve line items
                into the ERP, checks the total matches, saves it, and files the PDF in the right
                folder.
              </p>
              <p className="mt-3 text-lg leading-relaxed text-[var(--color-muted)]">
                Then does it again. Forty times a day.
              </p>
            </div>
            <div>
              <h2 className="text-xs font-medium tracking-widest text-[var(--color-muted)] uppercase">
                What Ghost does with it
              </h2>
              <p className="mt-4 text-lg leading-relaxed">
                Ghost does the retyping. It stops before saving, shows you the twelve lines it
                is about to write and the screenshot of where it will write them, and waits.
              </p>
              <p className="mt-3 text-lg leading-relaxed text-[var(--color-muted)]">
                You click Approve. It saves, confirms the total on screen matches, files the
                PDF, and writes down everything it touched.
              </p>
            </div>
          </div>
        </section>

        {/* ---------------------------------------------------------------- */}
        {/* How it works                                                     */}
        {/* ---------------------------------------------------------------- */}
        <section id="pipeline" className="scroll-mt-16 px-5 py-20">
          <div className="mx-auto max-w-6xl">
            <h2 className="text-2xl font-semibold tracking-tight">How it works</h2>
            <p className="mt-2 max-w-2xl text-sm text-[var(--color-muted)]">
              Seven things happen to every workflow, in this order.
            </p>

            <ol className="mt-10 space-y-px overflow-hidden rounded-xl border border-[var(--color-border)]">
              {PIPELINE.map((stage, i) => (
                <li
                  key={stage.name}
                  className="flex flex-col gap-1 bg-[var(--color-surface)] px-5 py-4 sm:flex-row sm:items-baseline sm:gap-6"
                >
                  <span className="w-6 shrink-0 font-mono text-xs text-[var(--color-muted)]">
                    {String(i + 1).padStart(2, "0")}
                  </span>
                  <span className="w-full shrink-0 text-sm font-medium sm:w-64">
                    {stage.name}
                    {!stage.built && (
                      <span className="ml-2 rounded bg-[var(--color-bg)] px-1.5 py-0.5 align-middle font-mono text-[10px] font-normal text-[var(--color-muted)]">
                        being built
                      </span>
                    )}
                  </span>
                  <span className="text-sm leading-relaxed text-[var(--color-muted)]">
                    {stage.blurb}
                  </span>
                </li>
              ))}
            </ol>
          </div>
        </section>

        {/* ---------------------------------------------------------------- */}
        {/* Approvals                                                        */}
        {/* ---------------------------------------------------------------- */}
        <section
          id="trust"
          className="scroll-mt-16 border-y border-[var(--color-border)] bg-[var(--color-surface)]/50 px-5 py-20"
        >
          <div className="mx-auto grid max-w-6xl gap-12 lg:grid-cols-2">
            <div>
              <h2 className="text-2xl font-semibold tracking-tight">
                What it will never do on its own
              </h2>
              <p className="mt-4 text-sm leading-relaxed text-[var(--color-muted)]">
                Ghost runs unattended right up until an action it can&rsquo;t take back. Then it
                stops. The list of actions that stop it is written in code as a fixed rule — an
                AI model never decides whether something is safe enough to skip asking you.
              </p>

              <ul className="mt-6 flex flex-wrap gap-2">
                {ALWAYS_STOPS.map((word) => (
                  <li
                    key={word}
                    className="rounded-lg border border-[var(--color-warning)]/40 bg-[var(--color-warning)]/10 px-3 py-1.5 text-sm font-medium"
                  >
                    {word}
                  </li>
                ))}
              </ul>

              <p className="mt-6 text-sm leading-relaxed text-[var(--color-muted)]">
                A run that&rsquo;s waiting stays waiting. It doesn&rsquo;t time out into
                proceeding, and an approval that sits too long expires rather than firing a
                payment days later.
              </p>
            </div>

            <div className="space-y-4">
              {[
                {
                  q: "What if it clicks the wrong thing?",
                  a: "It finds buttons by their visible label, not by screen position, so a moved button is still found and a renamed one fails loudly instead of clicking whatever is now in that spot.",
                },
                {
                  q: "What if the server dies mid-run?",
                  a: "Each run keeps a journal of completed steps. The next worker reads it and resumes after the last one that finished. Where it genuinely can't tell whether an action landed, it stops and asks rather than risking a double-submit.",
                },
                {
                  q: "Can someone edit the history?",
                  a: "Each log entry is hashed together with the previous one. Editing or deleting an old entry breaks the chain, and the Audit page recomputes it and points at where it broke.",
                },
                {
                  q: "Can it be undone?",
                  a: "Reversible steps can be walked back on request, and the log records the reversal as its own event rather than pretending the original never happened.",
                },
              ].map(({ q, a }) => (
                <div
                  key={q}
                  className="rounded-xl border border-[var(--color-border)] bg-[var(--color-bg)] px-5 py-4"
                >
                  <h3 className="text-sm font-medium">{q}</h3>
                  <p className="mt-1.5 text-sm leading-relaxed text-[var(--color-muted)]">{a}</p>
                </div>
              ))}
            </div>
          </div>
        </section>

        {/* ---------------------------------------------------------------- */}
        {/* Honest status                                                    */}
        {/* ---------------------------------------------------------------- */}
        <section id="status" className="scroll-mt-16 px-5 py-20">
          <div className="mx-auto max-w-6xl">
            <h2 className="text-2xl font-semibold tracking-tight">What&rsquo;s actually built</h2>
            <p className="mt-2 max-w-2xl text-sm text-[var(--color-muted)]">
              Working today, and what isn&rsquo;t yet. Listed plainly so nothing here has to be
              walked back later.
            </p>

            <div className="mt-10 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {BUILT.map(({ title, body }) => (
                <div
                  key={title}
                  className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5"
                >
                  <div className="flex items-center gap-2">
                    <span className="grid h-5 w-5 place-items-center rounded-full bg-[var(--color-success)]/15 text-[var(--color-success)]">
                      <Check className="h-3 w-3" strokeWidth={3} />
                    </span>
                    <h3 className="text-sm font-medium">{title}</h3>
                  </div>
                  <p className="mt-2 text-sm leading-relaxed text-[var(--color-muted)]">{body}</p>
                </div>
              ))}

              {NEXT_UP.map(({ title, body }) => (
                <div
                  key={title}
                  className="rounded-xl border border-dashed border-[var(--color-border)] p-5"
                >
                  <div className="flex items-center gap-2">
                    <span className="grid h-5 w-5 place-items-center rounded-full bg-[var(--color-muted)]/15 text-[var(--color-muted)]">
                      <Minus className="h-3 w-3" strokeWidth={3} />
                    </span>
                    <h3 className="text-sm font-medium text-[var(--color-muted)]">
                      {title} — not yet
                    </h3>
                  </div>
                  <p className="mt-2 text-sm leading-relaxed text-[var(--color-muted)]">{body}</p>
                </div>
              ))}
            </div>

            <p className="mt-8 font-mono text-xs text-[var(--color-muted)]">
              Next.js · Node worker · Postgres · Redis · Playwright · AGPL-3.0
            </p>
          </div>
        </section>

        {/* ---------------------------------------------------------------- */}
        {/* Closing CTA                                                      */}
        {/* ---------------------------------------------------------------- */}
        <section className="border-t border-[var(--color-border)] px-5 py-20">
          <div className="mx-auto max-w-2xl text-center">
            <h2 className="text-2xl font-semibold tracking-tight">
              Watch a run stop at its approval
            </h2>
            <p className="mt-3 text-sm leading-relaxed text-[var(--color-muted)]">
              The walkthrough follows the bundled demo workflow one step at a time — including
              the moment it refuses to click Submit.
            </p>
            <div className="mt-7 flex flex-wrap justify-center gap-3">
              <Link
                href="/demo"
                className="inline-flex h-10 items-center gap-2 rounded-lg bg-[var(--color-accent)] px-4 text-sm font-medium text-[var(--color-accent-fg)] transition-opacity hover:opacity-90"
              >
                See it step by step
                <ArrowRight className="h-4 w-4" />
              </Link>
              <Link
                href={signedIn ? "/dashboard" : "/signin"}
                className="inline-flex h-10 items-center rounded-lg border border-[var(--color-border)] px-4 text-sm font-medium transition-colors hover:bg-[var(--color-surface)]"
              >
                {signedIn ? "Open dashboard" : "Sign in"}
              </Link>
            </div>
          </div>
        </section>
      </main>

      <MarketingFooter />
    </>
  );
}
