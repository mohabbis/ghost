import { Check, ShieldAlert, Circle, Lock } from "lucide-react";
import { cn } from "@/lib/cn";
import { DEMO_STEPS, DEMO_DISCLAIMER, type DemoStep } from "@/lib/demo-narrative";

/**
 * The product in one image: a run stopped at its approval gate.
 *
 * Static markup, no client JS — the landing page's job is to load instantly
 * and say one true thing. The gate is the visual focus because it is the
 * single behaviour that distinguishes Ghost from a macro recorder.
 */

function StepIcon({ state }: { state: DemoStep["state"] }) {
  if (state === "succeeded") {
    return (
      <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-[var(--color-success)]/15 text-[var(--color-success)]">
        <Check className="h-3.5 w-3.5" strokeWidth={3} />
      </span>
    );
  }
  if (state === "approval") {
    return (
      <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-[var(--color-warning)]/20 text-[var(--color-warning)]">
        <ShieldAlert className="h-3.5 w-3.5" strokeWidth={2.5} />
      </span>
    );
  }
  return (
    <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full border border-dashed border-[var(--color-border)] text-[var(--color-muted)]">
      <Circle className="h-2 w-2" strokeWidth={4} />
    </span>
  );
}

export function RunPreview({ className }: { className?: string }) {
  return (
    <figure className={cn("m-0", className)}>
      <div className="overflow-hidden rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] shadow-2xl shadow-black/10">
        {/* Window chrome — signals "this is the product" without a screenshot
            that would go stale the moment the UI changes. */}
        <div className="flex items-center gap-2 border-b border-[var(--color-border)] px-4 py-2.5">
          <div className="flex gap-1.5" aria-hidden="true">
            <span className="h-2.5 w-2.5 rounded-full bg-[var(--color-border)]" />
            <span className="h-2.5 w-2.5 rounded-full bg-[var(--color-border)]" />
            <span className="h-2.5 w-2.5 rounded-full bg-[var(--color-border)]" />
          </div>
          <span className="ml-2 font-mono text-[11px] text-[var(--color-muted)]">
            run · demo: submit an order
          </span>
          <span className="ml-auto rounded-full bg-[var(--color-warning)]/15 px-2 py-0.5 text-[10px] font-medium tracking-wide text-[var(--color-warning)] uppercase">
            Awaiting approval
          </span>
        </div>

        <ol className="divide-y divide-[var(--color-border)]">
          {DEMO_STEPS.map((step, i) => (
            <li
              key={step.id}
              className={cn(
                "flex gap-3 px-4 py-3",
                step.state === "approval" && "bg-[var(--color-warning)]/[0.06]",
                step.state === "pending" && "opacity-55",
              )}
            >
              <StepIcon state={step.state} />
              <div className="min-w-0 flex-1">
                <div className="flex items-baseline gap-2">
                  <span className="font-mono text-[11px] text-[var(--color-muted)]">
                    {String(i + 1).padStart(2, "0")}
                  </span>
                  <span className="text-sm font-medium">{step.label}</span>
                  <span className="rounded bg-[var(--color-bg)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--color-muted)]">
                    {step.type}
                  </span>
                  {step.duration && (
                    <span className="ml-auto shrink-0 font-mono text-[11px] text-[var(--color-muted)]">
                      {step.duration}
                    </span>
                  )}
                </div>
                <p className="mt-1 text-xs leading-relaxed text-[var(--color-muted)]">
                  {step.detail}
                </p>

                {step.state === "approval" && (
                  <div className="mt-2.5 flex flex-wrap items-center gap-2">
                    <span className="inline-flex h-7 items-center rounded-lg bg-[var(--color-accent)] px-3 text-xs font-medium text-[var(--color-accent-fg)]">
                      Approve
                    </span>
                    <span className="inline-flex h-7 items-center rounded-lg border border-[var(--color-border)] px-3 text-xs font-medium">
                      Reject
                    </span>
                    <span className="inline-flex items-center gap-1 text-[11px] text-[var(--color-muted)]">
                      <Lock className="h-3 w-3" />
                      execution is blocked until someone decides
                    </span>
                  </div>
                )}
              </div>
            </li>
          ))}
        </ol>

        <div className="flex items-center justify-between gap-3 border-t border-[var(--color-border)] px-4 py-2.5">
          <span className="font-mono text-[11px] text-[var(--color-muted)]">
            audit chain · seq #1–#7
          </span>
          <span className="inline-flex items-center gap-1.5 text-[11px] font-medium text-[var(--color-success)]">
            <Check className="h-3 w-3" strokeWidth={3} />
            intact
          </span>
        </div>
      </div>

      <figcaption className="mt-2.5 text-center text-[11px] text-[var(--color-muted)]">
        {DEMO_DISCLAIMER}
      </figcaption>
    </figure>
  );
}
