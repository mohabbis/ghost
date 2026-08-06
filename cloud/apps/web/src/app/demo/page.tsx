import Link from "next/link";
import type { Metadata } from "next";
import { ArrowRight, Check, ShieldAlert, Link2, Camera, RotateCcw } from "lucide-react";
import { auth } from "@/auth";
import { MarketingNav, MarketingFooter } from "@/components/marketing/chrome";
import { DEMO_STEPS, DEMO_DISCLAIMER } from "@/lib/demo-narrative";

/**
 * A signed-out walkthrough of one run, start to finish.
 *
 * Exists because the interesting behaviour — the run stopping itself before an
 * action it can't take back — was only visible after signing in, creating an
 * org, seeding a workflow and triggering it. That is a lot of steps to ask of
 * someone deciding whether the product is real.
 *
 * Static and honest: this narrates the same four steps `demoWorkflowSteps()`
 * really executes, and says outright that it is an illustration rather than a
 * live run. It must never render live tenant data, and it must never be dressed
 * up to look like it did — `DEMO_DISCLAIMER` stays on the page.
 */

export const metadata: Metadata = {
  title: "Ghost — a run, step by step",
  description:
    "Follow one Ghost run from start to finish, including the moment it stops and refuses to click Submit without a person approving it.",
};

const AFTER_APPROVAL = [
  {
    icon: Check,
    title: "It clicks Submit",
    body: "Only now, and only because a person clicked Approve. The approval is single-use and tied to this exact run and step — it can't be replayed against a different one.",
  },
  {
    icon: Camera,
    title: "It screenshots the result",
    body: "The page as it looked immediately after the click, saved against this step so you can see what the browser saw.",
  },
  {
    icon: Check,
    title: 'It checks the page says "Order submitted"',
    body: "If that text isn't there, the run is marked failed. Ghost doesn't report success just because the click didn't throw an error.",
  },
  {
    icon: Link2,
    title: "It writes the log entry",
    body: "Run started, step finished, approval granted and by whom, run succeeded — each hashed together with the entry before it.",
  },
  {
    icon: RotateCcw,
    title: "It stays reversible",
    body: "If this order needed to be walked back, the reversal would be recorded as its own entry — the log never pretends the original didn't happen.",
  },
];

export default async function DemoPage() {
  const session = await auth();
  const signedIn = Boolean(session?.user);

  return (
    <>
      <MarketingNav signedIn={signedIn} />

      <main className="px-5 py-16">
        <div className="mx-auto max-w-3xl">
          <Link
            href="/"
            className="text-xs text-[var(--color-muted)] transition-colors hover:text-[var(--color-fg)]"
          >
            ← Back
          </Link>

          <h1 className="mt-5 text-3xl font-semibold tracking-tight sm:text-4xl">
            One run, step by step
          </h1>
          <p className="mt-4 leading-relaxed text-[var(--color-muted)]">
            This is the demo workflow that ships with Ghost. Four steps: open a form, type a
            name, submit it, confirm it went through. The third step is the interesting one.
          </p>
          <p className="mt-3 text-xs text-[var(--color-muted)]">{DEMO_DISCLAIMER}</p>

          {/* ------------------------------------------------------------- */}
          {/* The steps                                                     */}
          {/* ------------------------------------------------------------- */}
          <ol className="mt-12 space-y-4">
            {DEMO_STEPS.map((step, i) => {
              const gate = step.state === "approval";
              return (
                <li
                  key={step.id}
                  className={
                    gate
                      ? "rounded-xl border-2 border-[var(--color-warning)]/50 bg-[var(--color-warning)]/[0.07] p-5"
                      : "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5"
                  }
                >
                  <div className="flex items-center gap-3">
                    <span className="grid h-7 w-7 shrink-0 place-items-center rounded-full bg-[var(--color-bg)] font-mono text-xs font-medium">
                      {i + 1}
                    </span>
                    <h2 className="text-base font-medium">{step.label}</h2>
                    <span className="rounded bg-[var(--color-bg)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--color-muted)]">
                      {step.type}
                    </span>
                    {step.duration && (
                      <span className="ml-auto font-mono text-xs text-[var(--color-muted)]">
                        {step.duration}
                      </span>
                    )}
                  </div>

                  <p className="mt-3 text-sm leading-relaxed text-[var(--color-muted)]">
                    {step.detail}
                  </p>

                  {gate && (
                    <div className="mt-5 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
                      <div className="flex items-center gap-2 text-sm font-medium text-[var(--color-warning)]">
                        <ShieldAlert className="h-4 w-4" />
                        The run stops here
                      </div>
                      <p className="mt-2 text-sm leading-relaxed text-[var(--color-muted)]">
                        Nothing has been submitted. The browser is sitting on the form with the
                        name filled in, and Ghost is waiting. It will wait indefinitely — there
                        is no timeout that lets it proceed on its own.
                      </p>
                      <p className="mt-2 text-sm leading-relaxed text-[var(--color-muted)]">
                        Whoever approves sees the step, the screenshot, and what the click will
                        do. If the workflow is set to require a second person, whoever started
                        the run can&rsquo;t be the one to approve it.
                      </p>
                      <div className="mt-4 flex flex-wrap items-center gap-2">
                        <span className="inline-flex h-8 items-center rounded-lg bg-[var(--color-accent)] px-3 text-xs font-medium text-[var(--color-accent-fg)]">
                          Approve
                        </span>
                        <span className="inline-flex h-8 items-center rounded-lg border border-[var(--color-border)] px-3 text-xs font-medium">
                          Reject
                        </span>
                        <span className="text-[11px] text-[var(--color-muted)]">
                          Rejecting stops the run. It is never restricted to a second person —
                          anyone who can see a runaway run must be able to halt it.
                        </span>
                      </div>
                    </div>
                  )}
                </li>
              );
            })}
          </ol>

          {/* ------------------------------------------------------------- */}
          {/* After approval                                                */}
          {/* ------------------------------------------------------------- */}
          <section className="mt-16">
            <h2 className="text-xl font-semibold tracking-tight">Once someone approves</h2>
            <div className="mt-6 space-y-3">
              {AFTER_APPROVAL.map(({ icon: Icon, title, body }) => (
                <div
                  key={title}
                  className="flex gap-4 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5"
                >
                  <span className="grid h-8 w-8 shrink-0 place-items-center rounded-lg bg-[var(--color-bg)] text-[var(--color-muted)]">
                    <Icon className="h-4 w-4" />
                  </span>
                  <div>
                    <h3 className="text-sm font-medium">{title}</h3>
                    <p className="mt-1.5 text-sm leading-relaxed text-[var(--color-muted)]">
                      {body}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          </section>

          {/* ------------------------------------------------------------- */}
          {/* If it goes wrong                                              */}
          {/* ------------------------------------------------------------- */}
          <section className="mt-16">
            <h2 className="text-xl font-semibold tracking-tight">If something goes wrong</h2>
            <div className="mt-6 space-y-3">
              {[
                [
                  "The button isn't there any more",
                  'The step fails and the run stops. Ghost looks for a button labelled "Submit order" — it does not click whatever now sits at those coordinates.',
                ],
                [
                  "The worker crashes after clicking Submit",
                  "The order may or may not have gone through, and Ghost won't guess. The step is marked unknown, the run raises an incident, and a person decides whether to retry or skip.",
                ],
                [
                  "The confirmation never appears",
                  "The check on step 4 fails, so the run is marked failed — even though every click succeeded. A run isn't finished until the outcome is confirmed.",
                ],
              ].map(([title, body]) => (
                <div
                  key={title}
                  className="rounded-xl border border-[var(--color-border)] p-5"
                >
                  <h3 className="text-sm font-medium">{title}</h3>
                  <p className="mt-1.5 text-sm leading-relaxed text-[var(--color-muted)]">
                    {body}
                  </p>
                </div>
              ))}
            </div>
          </section>

          <div className="mt-16 flex flex-wrap items-center gap-3 border-t border-[var(--color-border)] pt-10">
            <Link
              href={signedIn ? "/dashboard" : "/signin"}
              className="inline-flex h-10 items-center gap-2 rounded-lg bg-[var(--color-accent)] px-4 text-sm font-medium text-[var(--color-accent-fg)] transition-opacity hover:opacity-90"
            >
              {signedIn ? "Open dashboard" : "Sign in and run it"}
              <ArrowRight className="h-4 w-4" />
            </Link>
            <Link
              href="/"
              className="inline-flex h-10 items-center rounded-lg border border-[var(--color-border)] px-4 text-sm font-medium transition-colors hover:bg-[var(--color-surface)]"
            >
              Back to overview
            </Link>
          </div>
        </div>
      </main>

      <MarketingFooter />
    </>
  );
}
