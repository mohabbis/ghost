import Link from "next/link";
import { redirect } from "next/navigation";
import { auth } from "@/auth";
import { SOURCE_URL } from "@/lib/source-url";

/**
 * Public landing page. Signed-in visitors go straight to their dashboard;
 * everyone else sees what Ghost actually does — and why — before being
 * asked to sign in.
 *
 * Every claim here is scoped to what the product currently does (the
 * record → review → approve → execute → verify → audit loop, verified
 * end-to-end against the seeded demo workflow). No fabricated logos,
 * customer counts, or capabilities ahead of what's built — see Engineering
 * rule 10 in the root CLAUDE.md.
 */

const PIPELINE = [
  {
    stage: "Capture",
    detail:
      "Record a workflow once in the browser — clicks, typed values, selects, and navigation, captured by role and name rather than brittle coordinates.",
  },
  {
    stage: "Review",
    detail:
      "The recording compiles into typed, editable steps. Nothing runs until a human has read the plan.",
  },
  {
    stage: "Approve",
    detail:
      "Sends, payments, deletes, submits — anything sensitive halts and waits for an explicit approval. Deny by default; AI never grants its own request.",
  },
  {
    stage: "Execute",
    detail: "Deterministic code replays the approved plan, step by step, against the real target.",
  },
  {
    stage: "Verify",
    detail:
      "Each step's outcome is checked against what it was supposed to do — a screenshot and an assertion, not an assumption.",
  },
  {
    stage: "Audit",
    detail:
      "Every run, approval, and mutation is appended to a hash-chained log, so tampering with the history is detectable, not just discouraged.",
  },
  {
    stage: "Recover",
    detail:
      "Reversible steps carry their own undo. A run that fails partway leaves a record of exactly what already happened — never a guess.",
  },
] as const;

const NOT_LIST = [
  "Another chatbot",
  "A no-code automation builder",
  "A macro recorder",
  "A generic AI assistant",
];

const VERTICALS = [
  "Wholesale distribution",
  "Property management",
  "Accounting & bookkeeping",
  "Logistics",
  "Recruiting",
  "Financial operations",
  "Healthcare admin",
];

export default async function Home() {
  const session = await auth();
  if (session) redirect("/dashboard");

  return (
    <main className="relative overflow-hidden">
      {/* Faint schematic grid — atmosphere for a product whose entire premise
          is precision, without spending any real color on it. */}
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0 -z-10 opacity-[0.05]"
        style={{
          backgroundImage:
            "linear-gradient(to right, var(--color-fg) 1px, transparent 1px), linear-gradient(to bottom, var(--color-fg) 1px, transparent 1px)",
          backgroundSize: "48px 48px",
        }}
      />

      {/* ---------- Nav ---------- */}
      <header className="mx-auto flex max-w-5xl items-center justify-between px-6 py-6">
        <span className="text-sm font-medium tracking-tight">Ghost</span>
        <Link
          href="/signin"
          className="inline-flex h-8 items-center justify-center rounded-lg border border-[var(--color-border)] px-3 text-sm font-medium transition-colors hover:bg-[var(--color-surface)]"
        >
          Sign in
        </Link>
      </header>

      {/* ---------- Hero ---------- */}
      <section className="mx-auto max-w-3xl px-6 pt-16 pb-20 text-center sm:pt-24 sm:pb-28">
        <p
          className="animate-[fade-up_0.6s_ease-out_both] text-xs font-medium tracking-[0.2em] text-[var(--color-accent)] uppercase"
          style={{ fontFamily: "var(--font-technical)" }}
        >
          Governed execution for AI agents
        </p>
        <h1
          className="mt-5 animate-[fade-up_0.6s_ease-out_0.08s_both] text-4xl leading-[1.1] font-normal text-balance sm:text-6xl"
          style={{ fontFamily: "var(--font-display)" }}
        >
          Teach Ghost a workflow <em className="italic">once.</em>
        </h1>
        <p className="mx-auto mt-6 max-w-xl animate-[fade-up_0.6s_ease-out_0.16s_both] text-base text-[var(--color-muted)] sm:text-lg">
          It executes reliably, pauses for approval on sensitive steps, verifies the outcome, and
          logs exactly what changed — an AI operator that runs across the software you already
          use, not another app that replaces it.
        </p>
        <div className="mt-9 flex animate-[fade-up_0.6s_ease-out_0.24s_both] flex-wrap items-center justify-center gap-3">
          <Link
            href="/signin"
            className="inline-flex h-11 items-center justify-center rounded-lg bg-[var(--color-accent)] px-6 text-sm font-medium text-[var(--color-accent-fg)] transition-transform hover:scale-[1.02] active:scale-[0.98]"
          >
            Get started
          </Link>
          <a
            href="#pipeline"
            className="inline-flex h-11 items-center justify-center rounded-lg px-6 text-sm font-medium text-[var(--color-muted)] transition-colors hover:text-[var(--color-fg)]"
          >
            See how it works ↓
          </a>
        </div>
      </section>

      {/* ---------- Problem ---------- */}
      <section className="mx-auto max-w-3xl px-6 pb-20 sm:pb-28">
        <p className="text-center text-lg text-[var(--color-fg)] sm:text-xl">
          Operations-heavy businesses move information by hand between disconnected systems —
          email, PDFs, Excel, ERPs, CRMs, web portals, legacy desktop apps.
        </p>
        <p className="mt-3 text-center text-lg text-[var(--color-muted)] sm:text-xl">
          That work is repetitive, error-prone, and expensive. Ghost automates it with controls a
          regulated business will actually accept.
        </p>
      </section>

      {/* ---------- Pipeline ---------- */}
      <section id="pipeline" className="mx-auto max-w-2xl scroll-mt-10 px-6 pb-20 sm:pb-28">
        <h2
          className="text-center text-2xl sm:text-3xl"
          style={{ fontFamily: "var(--font-display)" }}
        >
          One engine, seven checkpoints.
        </h2>
        <p className="mx-auto mt-3 max-w-md text-center text-sm text-[var(--color-muted)]">
          The same trust pipeline runs every workflow, from the first recorded click to the last
          audited byte.
        </p>

        <ol className="mt-14 space-y-0">
          {PIPELINE.map((step, i) => (
            <li key={step.stage} className="relative flex gap-5 pb-10 last:pb-0">
              {i < PIPELINE.length - 1 && (
                <span
                  aria-hidden
                  className="absolute top-8 left-[15px] h-full w-px bg-[var(--color-border)]"
                />
              )}
              <span
                className="relative z-10 flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-[var(--color-border)] bg-[var(--color-bg)] text-xs text-[var(--color-muted)]"
                style={{ fontFamily: "var(--font-technical)" }}
              >
                {String(i + 1).padStart(2, "0")}
              </span>
              <div className="pt-0.5">
                <div
                  className="text-base font-medium"
                  style={{ fontFamily: "var(--font-technical)" }}
                >
                  {step.stage}
                </div>
                <p className="mt-1 max-w-md text-sm text-[var(--color-muted)]">{step.detail}</p>
              </div>
            </li>
          ))}
        </ol>
      </section>

      {/* ---------- Positioning ---------- */}
      <section className="mx-auto max-w-3xl px-6 pb-20 sm:pb-28">
        <div className="grid grid-cols-1 gap-6 sm:grid-cols-2">
          <div className="rounded-lg border border-[var(--color-accent)]/30 bg-[var(--color-accent)]/[0.06] p-6">
            <div
              className="text-xs tracking-[0.15em] text-[var(--color-accent)] uppercase"
              style={{ fontFamily: "var(--font-technical)" }}
            >
              Ghost is
            </div>
            <p className="mt-3 text-lg" style={{ fontFamily: "var(--font-display)" }}>
              An execution platform. An AI operator.
            </p>
            <p className="mt-2 text-sm text-[var(--color-muted)]">
              Demonstrate a real process once; Ghost executes, adapts, verifies, and escalates
              exceptions.
            </p>
          </div>
          <div className="rounded-lg border border-[var(--color-border)] p-6">
            <div
              className="text-xs tracking-[0.15em] text-[var(--color-muted)] uppercase"
              style={{ fontFamily: "var(--font-technical)" }}
            >
              Ghost is not
            </div>
            <ul className="mt-3 space-y-1.5 text-sm text-[var(--color-muted)]">
              {NOT_LIST.map((item) => (
                <li key={item}>{item}</li>
              ))}
            </ul>
          </div>
        </div>
      </section>

      {/* ---------- Who it's for ---------- */}
      <section className="mx-auto max-w-3xl px-6 pb-24 text-center sm:pb-32">
        <p className="text-sm text-[var(--color-muted)]">Built for operations-heavy teams —</p>
        <div className="mt-4 flex flex-wrap items-center justify-center gap-2">
          {VERTICALS.map((v) => (
            <span
              key={v}
              className="rounded-full border border-[var(--color-border)] px-3 py-1 text-xs text-[var(--color-muted)]"
            >
              {v}
            </span>
          ))}
        </div>
      </section>

      {/* ---------- Final CTA ---------- */}
      <section className="mx-auto max-w-3xl px-6 pb-24 text-center sm:pb-32">
        <p className="text-2xl sm:text-3xl" style={{ fontFamily: "var(--font-display)" }}>
          Recurring, measurable, reversible work — <em className="italic">handled.</em>
        </p>
        <Link
          href="/signin"
          className="mt-8 inline-flex h-11 items-center justify-center rounded-lg bg-[var(--color-accent)] px-6 text-sm font-medium text-[var(--color-accent-fg)] transition-transform hover:scale-[1.02] active:scale-[0.98]"
        >
          Get started
        </Link>
      </section>

      {/*
        Also offered here, not only behind the sidebar: AGPL-3.0 section 13 covers
        everyone interacting with this instance over the network, and someone who
        never signs in never sees the authenticated chrome.
      */}
      <footer className="mx-auto max-w-3xl px-6 pb-16 text-center">
        <a
          href={SOURCE_URL}
          target="_blank"
          rel="noreferrer"
          className="text-xs text-[var(--color-muted)] hover:text-[var(--color-fg)]"
        >
          AGPL-3.0 · Source
        </a>
      </footer>
    </main>
  );
}
