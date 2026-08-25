"use client";

import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardBody } from "@/components/ui/card";

interface StepView {
  index: number;
  type: string;
  label: string | null;
  status: string;
  screenshotUrl: string | null;
  verification: { passed: boolean; detail: string } | null;
  error: string | null;
  attempt: number;
  output: Record<string, string> | null;
}
interface ApprovalView {
  stepIndex: number;
  status: string;
  reason: string;
  expiresAt: string | null;
  phase: "FORWARD" | "COMPENSATION";
  /** For a compensation gate: what the reversal will actually do. */
  actions: string[];
}
interface RunView {
  id: string;
  status: string;
  error: string | null;
  workflowName: string;
  cursor: number;
  restoreScreenshotUrl: string | null;
  restoreOutcome: string | null;
  /** Null only if the run has never executed a step. */
  startedAt: string | null;
  steps: StepView[];
  approvals: ApprovalView[];
  /** Set when the run is queued behind its workflow's concurrency cap. */
  throttle: { reason: string; activeRunIds: string[] } | null;
  /**
   * False when this workflow requires a separate approver and the viewer is the
   * person who started the run. Advisory only — the route refuses regardless;
   * this exists so nobody discovers the rule by having a click rejected.
   */
  canApprove: boolean;
  /** Deterministic disposition of the stop. Non-null only for INCIDENT. */
  exception: {
    kind: string;
    owner: string;
    headline: string;
    guidance: string;
    retryUseful: boolean;
    retryMayDuplicate: boolean;
    /** When this incident was raised — echoed back to bind the confirmation. */
    raisedAt: string | null;
  } | null;
}
interface ChainView {
  org: { intact: boolean; count: number };
  run?: { intact: boolean; count: number; anchored: boolean; anchorMatches: boolean | null };
}
interface UndoView {
  canUndo: boolean;
  complete: boolean;
  /** Steps still landing. Non-empty means the plan is not yet trustworthy. */
  blockedBy: number[] | null;
  alreadyReversed: number[];
  entries: {
    stepIndex: number;
    stepLabel: string;
    description: string;
    /** What the reversal will actually do, not just what its author called it. */
    actions: string[];
    requiresApproval: boolean;
    approvalReason: string | null;
  }[];
  irreversible: { stepIndex: number; stepLabel: string; reason: string }[];
}

// Stops polling and triggers chain verification. COMPENSATED belongs here:
// leaving it out kept the page polling forever and meant the verification
// effect never re-ran, so the panel kept showing the pre-undo audit result
// instead of verifying the head the compensation just anchored.
const TERMINAL = new Set(["SUCCEEDED", "FAILED", "CANCELED", "COMPENSATED"]);
/** States a run can be reversed from. Mirrors the undo route's allow-list. */
const REVERSIBLE = new Set(["SUCCEEDED", "FAILED", "CANCELED", "INCIDENT"]);
// COMPENSATING included: a reversal is a sequence of live mutations and needs
// the same stop the forward direction has.
const STOPPABLE = new Set([
  "QUEUED",
  "RUNNING",
  "AWAITING_APPROVAL",
  "INCIDENT",
  "COMPENSATING",
]);

function statusColor(status: string): string {
  if (status === "SUCCEEDED") return "text-[var(--color-success)]";
  if (status === "FAILED" || status === "CANCELED") return "text-[var(--color-danger)]";
  // An indeterminate outcome is not a failure and must not read like one — it
  // means nobody knows whether the action took effect.
  if (status === "UNKNOWN" || status === "INCIDENT") return "text-[var(--color-warning)]";
  if (status === "AWAITING_APPROVAL") return "text-[var(--color-warning)]";
  // A reversed step is neither a success nor a failure — it happened, and then
  // it was walked back.
  if (status === "COMPENSATED" || status === "COMPENSATING") {
    return "text-[var(--color-muted)]";
  }
  return "text-[var(--color-muted)]";
}

export function RunTimeline({ runId }: { runId: string }) {
  const [run, setRun] = useState<RunView | null>(null);
  const [chain, setChain] = useState<ChainView | null>(null);
  const [undo, setUndo] = useState<UndoView | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  // Two-step confirmation for a retry whose effect may already have happened.
  //
  // Holds the *identity* of the incident it was opened for, not just a boolean.
  // The page is fed by SSE, so `run` changes underneath an open dialog: if
  // another operator resolves this incident and the run then parks on a new
  // risky step, a bare flag would let the still-open button acknowledge a step
  // its clicker never read. The route refuses a mismatch; this keeps the UI
  // honest too, by closing the dialog the moment it stops describing what is
  // actually on screen.
  const [confirmRetry, setConfirmRetry] = useState<
    { stepIndex: number; raisedAt: string | null } | null
  >(null);
  // Same two-step confirmation for Undo. The route refuses to reschedule a
  // reversal whose previous attempt may already have landed, and without this
  // the Undo button — which posts `{}` — could never satisfy it, leaving a
  // partially completed reversal unresumable from the browser.
  const [confirmRevert, setConfirmRevert] = useState<
    { stepIndex: number; raisedAt: string | null } | null
  >(null);

  const load = useCallback(async () => {
    const res = await fetch(`/api/runs/${runId}`, { cache: "no-store" });
    if (res.ok) setRun((await res.json()) as RunView);
  }, [runId]);

  const loadUndo = useCallback(async () => {
    const res = await fetch(`/api/runs/${runId}/undo`, { cache: "no-store" });
    if (res.ok) setUndo((await res.json()) as UndoView);
  }, [runId]);

  const loadChain = useCallback(async () => {
    const res = await fetch(`/api/audit/verify?runId=${runId}`, { cache: "no-store" });
    if (res.ok) setChain((await res.json()) as ChainView);
  }, [runId]);

  // Pushed over SSE (`/api/runs/[id]/stream`) instead of polled. That route
  // deliberately ends each connection well inside Vercel's function time
  // limit rather than holding one open for a run's whole lifetime — see its
  // doc comment — so a *closed* stream reconnects on its own by design.
  // Closing here on a terminal status is what actually stops that cycle: a
  // clean end otherwise looks identical to a dropped connection to
  // `EventSource`, which retries either way.
  useEffect(() => {
    void load();

    const source = new EventSource(`/api/runs/${runId}/stream`);
    source.onmessage = (event) => {
      const data = JSON.parse(event.data as string) as RunView | { notFound: true };
      if ("notFound" in data) {
        source.close();
        return;
      }
      setRun(data);
      if (TERMINAL.has(data.status)) source.close();
    };

    return () => source.close();
  }, [runId, load]);

  // Verify the chain once the run stops changing.
  useEffect(() => {
    if (run && TERMINAL.has(run.status)) void loadChain();
  }, [run, loadChain]);

  // The undo plan is read-only, so it can be fetched as soon as the run is in a
  // state that could be reversed.
  useEffect(() => {
    if (run && REVERSIBLE.has(run.status)) void loadUndo();
  }, [run, loadUndo]);

  async function post(path: string, body: unknown) {
    setBusy(true);
    setNotice(null);
    try {
      const res = await fetch(path, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      });
      if (!res.ok) {
        const data = (await res.json().catch(() => ({}))) as { error?: string };
        setNotice(data.error ?? `Request failed (${res.status})`);
      }
      await load();
      await loadUndo().catch(() => undefined);
    } finally {
      setBusy(false);
    }
  }

  if (!run) return <p className="text-sm text-[var(--color-muted)]">Loading run…</p>;

  const pending = run.approvals.find((a) => a.status === "PENDING");
  const incidentStep = run.status === "INCIDENT" ? run.steps.find((s) => s.index === run.cursor) : undefined;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">{run.workflowName}</h1>
          <p className="mt-1 font-mono text-xs text-[var(--color-muted)]">{run.id}</p>
        </div>
        <div className="flex items-center gap-3">
          <span className={`text-sm font-medium ${statusColor(run.status)}`}>{run.status}</span>
          {STOPPABLE.has(run.status) && (
            <Button
              size="sm"
              variant="danger"
              disabled={busy}
              onClick={() => post(`/api/runs/${run.id}/cancel`, {})}
            >
              {run.status === "COMPENSATING"
                ? "Stop after current reversal"
                : "Stop after current step"}
            </Button>
          )}
        </div>
      </div>

      {run.error && <p className="text-sm text-[var(--color-danger)]">{run.error}</p>}
      {notice && <p className="text-sm text-[var(--color-danger)]">{notice}</p>}

      {run.throttle && (
        <Card>
          <CardBody className="space-y-2">
            <div className="text-sm">
              <span className="font-medium">Waiting its turn</span> — {run.throttle.reason}
            </div>
            <p className="text-xs text-[var(--color-muted)]">
              {run.startedAt
                ? // A run retried from an incident is waiting again, but its
                  // earlier steps really happened — telling an operator nothing
                  // has run would be false, and about side effects.
                  "This run has already executed some steps. It continues as soon as one of the runs ahead of it finishes."
                : "Nothing has run yet. This run starts as soon as one of the runs ahead of it finishes."}
            </p>
            {run.throttle.activeRunIds.length > 0 && (
              <ul className="space-y-0.5 text-xs">
                {run.throttle.activeRunIds.map((id) => (
                  <li key={id}>
                    <a className="underline underline-offset-2" href={`/runs/${id}`}>
                      Run {id.slice(0, 8)}
                    </a>{" "}
                    <span className="text-[var(--color-muted)]">holds a slot</span>
                  </li>
                ))}
              </ul>
            )}
          </CardBody>
        </Card>
      )}

      {pending && (
        <Card>
          <CardBody className="flex items-start justify-between gap-4">
            <div className="text-sm">
              <span className="font-medium">
                {pending.phase === "COMPENSATION"
                  ? "Approval required to reverse a step"
                  : "Approval required"}
              </span>{" "}
              — {pending.reason}
              {!run.canApprove && (
                <div className="mt-1.5 text-xs text-[var(--color-muted)]">
                  You started this run. Someone else in your organization has to approve it —
                  you can still reject it.
                </div>
              )}
              {pending.expiresAt && (
                <span className="ml-2 text-xs text-[var(--color-muted)]">
                  expires {new Date(pending.expiresAt).toLocaleString()}
                </span>
              )}
              {pending.actions.length > 0 && (
                <ul className="mt-1.5 ml-4 list-disc space-y-0.5 font-mono text-xs text-[var(--color-muted)] marker:text-[var(--color-border)]">
                  {pending.actions.map((a, i) => (
                    <li key={i}>{a}</li>
                  ))}
                </ul>
              )}
            </div>
            <div className="flex shrink-0 gap-2">
              <Button
                size="sm"
                disabled={busy || !run.canApprove}
                title={
                  run.canApprove
                    ? undefined
                    : "You started this run, and this workflow requires someone else to approve."
                }
                onClick={() =>
                  post(`/api/runs/${run.id}/approvals/${pending.stepIndex}`, { decision: "approve" })
                }
              >
                Approve
              </Button>
              <Button
                size="sm"
                variant="danger"
                disabled={busy}
                onClick={() =>
                  post(`/api/runs/${run.id}/approvals/${pending.stepIndex}`, { decision: "reject" })
                }
              >
                Reject
              </Button>
            </div>
          </CardBody>
        </Card>
      )}

      {run.status === "INCIDENT" && (
        <Card>
          <CardBody className="space-y-3">
            <div className="text-sm">
              <span className="font-medium text-[var(--color-warning)]">
                Stopped at step {run.cursor + 1}
              </span>
              {incidentStep?.label ? ` — ${incidentStep.label}` : ""}
            </div>
            {run.exception && (
              <div className="space-y-1">
                <p
                  className={
                    run.exception.retryMayDuplicate
                      ? "text-xs font-medium text-[var(--color-danger)]"
                      : "text-xs font-medium text-[var(--color-warning)]"
                  }
                >
                  {run.exception.headline}
                </p>
                <p className="text-xs text-[var(--color-muted)]">{run.exception.guidance}</p>
              </div>
            )}
            {run.restoreScreenshotUrl && (
              <div className="space-y-1">
                <p className="text-xs text-[var(--color-muted)]">
                  {run.restoreOutcome === "session.restore_failed"
                    ? "The page as Ghost found it when it could not rebuild the session:"
                    : "The page after Ghost rebuilt the session:"}
                </p>
                {/* eslint-disable-next-line @next/next/no-img-element */}
                <img
                  src={run.restoreScreenshotUrl}
                  alt="page state after restore"
                  className="max-h-64 rounded border border-[var(--color-border)]"
                />
              </div>
            )}
            {/*
              Retry is de-emphasised when the classifier says it cannot help (a
              changed selector fails identically) and becomes a two-step
              confirmation when it could repeat an effect that already happened.
              The route enforces the same rule — it refuses an unacknowledged
              risky retry with a 409 — so this is the honest affordance for a
              decision the server will demand anyway, not a client-side guard
              standing in for one.
            */}
            {confirmRetry &&
            confirmRetry.stepIndex === run.cursor &&
            confirmRetry.raisedAt === (run.exception?.raisedAt ?? null) ? (
              <div className="space-y-2 rounded border border-[var(--color-danger)] p-2">
                <p className="text-xs font-medium text-[var(--color-danger)]">
                  Retry anyway? This step may already have taken effect, and retrying could repeat
                  it. Confirm in the target system first.
                </p>
                <div className="flex gap-2">
                  <Button
                    size="sm"
                    variant="danger"
                    disabled={busy}
                    onClick={() => {
                      const confirmed = confirmRetry;
                      setConfirmRetry(null);
                      void post(`/api/runs/${run.id}/incident`, {
                        action: "retry",
                        acknowledgeDuplicateRisk: true,
                        // What the human actually read. The route rejects the
                        // acknowledgement if the run has since stopped
                        // somewhere else.
                        expectStepIndex: confirmed.stepIndex,
                        ...(confirmed.raisedAt
                          ? { expectIncidentRaisedAt: confirmed.raisedAt }
                          : {}),
                      });
                    }}
                  >
                    Yes, retry — I checked
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy}
                    onClick={() => setConfirmRetry(null)}
                  >
                    Cancel
                  </Button>
                </div>
              </div>
            ) : (
              <div className="flex gap-2">
                <Button
                  size="sm"
                  variant={run.exception?.retryUseful === false ? "ghost" : "primary"}
                  disabled={busy}
                  onClick={() => {
                    if (run.exception?.retryMayDuplicate) {
                      setConfirmRetry({
                        stepIndex: run.cursor,
                        raisedAt: run.exception.raisedAt ?? null,
                      });
                      return;
                    }
                    void post(`/api/runs/${run.id}/incident`, { action: "retry" });
                  }}
                >
                  Retry step
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onClick={() => post(`/api/runs/${run.id}/incident`, { action: "skip" })}
                >
                  Skip step
                </Button>
              </div>
            )}
          </CardBody>
        </Card>
      )}

      {run && REVERSIBLE.has(run.status) && undo && (undo.entries.length > 0 || undo.irreversible.length > 0) && (
        <Card>
          <CardBody className="space-y-3">
            <div className="flex items-start justify-between gap-4">
              <div>
                <p className="text-sm font-medium">Undo this run</p>
                <p className="mt-1 text-xs text-[var(--color-muted)]">
                  {undo.blockedBy
                    ? `Step ${undo.blockedBy[0]! + 1} is still finishing. Undo is unavailable until the run settles.`
                    : undo.entries.length === 0
                      ? "Nothing left in this run can be reversed."
                      : `${undo.entries.length} step${undo.entries.length === 1 ? "" : "s"} would be reversed, newest first.`}
                  {undo.alreadyReversed.length > 0 &&
                    ` ${undo.alreadyReversed.length} step${
                      undo.alreadyReversed.length === 1 ? " has" : "s have"
                    } already been reversed.`}
                </p>
              </div>
              {undo.entries.length > 0 && undo.canUndo && (
                <Button
                  size="sm"
                  variant="danger"
                  disabled={busy}
                  onClick={() => {
                    if (run.status === "INCIDENT" && run.exception?.retryMayDuplicate) {
                      setConfirmRevert({
                        stepIndex: run.cursor,
                        raisedAt: run.exception.raisedAt ?? null,
                      });
                      return;
                    }
                    void post(`/api/runs/${run.id}/undo`, {});
                  }}
                >
                  Undo run
                </Button>
              )}
            </div>
            {confirmRevert &&
              confirmRevert.stepIndex === run.cursor &&
              confirmRevert.raisedAt === (run.exception?.raisedAt ?? null) && (
                <div className="mt-2 space-y-2 rounded border border-[var(--color-danger)] p-2">
                  <p className="text-xs font-medium text-[var(--color-danger)]">
                    A previous reversal of this run may already have taken effect before it
                    failed. Reversing again could repeat it. Confirm in the target system first.
                  </p>
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="danger"
                      disabled={busy}
                      onClick={() => {
                        const confirmed = confirmRevert;
                        setConfirmRevert(null);
                        void post(`/api/runs/${run.id}/undo`, {
                          acknowledgeDuplicateRisk: true,
                          expectStepIndex: confirmed.stepIndex,
                          ...(confirmed.raisedAt
                            ? { expectIncidentRaisedAt: confirmed.raisedAt }
                            : {}),
                        });
                      }}
                    >
                      Yes, reverse again — I checked
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={busy}
                      onClick={() => setConfirmRevert(null)}
                    >
                      Cancel
                    </Button>
                  </div>
                </div>
              )}

            {undo.entries.length > 0 && (
              <ol className="space-y-2 text-xs">
                {undo.entries.map((e) => (
                  <li key={e.stepIndex} className="text-[var(--color-muted)]">
                    <span className="text-[var(--color-fg)]">{e.stepLabel}</span> — {e.description}
                    {e.requiresApproval && (
                      <span className="ml-1 text-[var(--color-warning)]">(needs approval)</span>
                    )}
                    {/* The description is the author's claim; this is the plan.
                        Approving a reversal you cannot see is not approval. */}
                    {e.actions.length > 0 && (
                      <ul className="mt-0.5 ml-3 list-disc space-y-0.5 marker:text-[var(--color-border)]">
                        {e.actions.map((a, i) => (
                          <li key={i} className="font-mono">
                            {a}
                          </li>
                        ))}
                      </ul>
                    )}
                  </li>
                ))}
              </ol>
            )}

            {undo.irreversible.length > 0 && (
              <div className="rounded border border-[var(--color-border)] p-2">
                <p className="text-xs font-medium text-[var(--color-warning)]">
                  Cannot be reversed — {undo.irreversible.length} completed step
                  {undo.irreversible.length === 1 ? "" : "s"}
                </p>
                <ul className="mt-1 space-y-0.5 text-xs text-[var(--color-muted)]">
                  {undo.irreversible.map((e) => (
                    <li key={e.stepIndex}>
                      {e.stepLabel} — {e.reason}
                    </li>
                  ))}
                </ul>
                <p className="mt-1 text-xs text-[var(--color-muted)]">
                  Undoing this run will leave those effects in place.
                </p>
              </div>
            )}
          </CardBody>
        </Card>
      )}

      <ol className="space-y-2">
        {run.steps.map((s) => (
          <li key={s.index}>
            <Card>
              <CardBody className="flex gap-4">
                <div className="w-6 shrink-0 text-sm text-[var(--color-muted)]">{s.index + 1}</div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-medium">{s.label ?? s.type}</span>
                    <span className={`text-xs ${statusColor(s.status)}`}>
                      {s.status}
                      {s.attempt > 1 && ` (attempt ${s.attempt})`}
                    </span>
                  </div>
                  {s.verification && (
                    <div
                      className={`mt-1 text-xs ${
                        s.verification.passed
                          ? "text-[var(--color-success)]"
                          : "text-[var(--color-danger)]"
                      }`}
                    >
                      verify: {s.verification.detail}
                    </div>
                  )}
                  {s.output && Object.keys(s.output).length > 0 && (
                    <div className="mt-1 font-mono text-xs text-[var(--color-muted)]">
                      {Object.entries(s.output)
                        .map(([k, v]) => `${k}: ${v}`)
                        .join("  ")}
                    </div>
                  )}
                  {s.error && (
                    <div className="mt-1 text-xs text-[var(--color-danger)]">{s.error}</div>
                  )}
                </div>
                {s.screenshotUrl && (
                  // eslint-disable-next-line @next/next/no-img-element
                  <img
                    src={s.screenshotUrl}
                    alt={`step ${s.index + 1} screenshot`}
                    className="h-16 w-28 shrink-0 rounded border border-[var(--color-border)] object-cover"
                  />
                )}
              </CardBody>
            </Card>
          </li>
        ))}
        {run.steps.length === 0 && (
          <p className="text-sm text-[var(--color-muted)]">Waiting for the first step…</p>
        )}
      </ol>

      {chain?.run && (
        <p className="text-xs text-[var(--color-muted)]">
          Audit chain:{" "}
          <span
            className={
              chain.run.intact
                ? "text-[var(--color-success)]"
                : "text-[var(--color-danger)]"
            }
          >
            {chain.run.intact ? "intact" : "BROKEN"}
          </span>{" "}
          across {chain.run.count} journal entries
          {chain.run.anchored &&
            (chain.run.anchorMatches
              ? ", anchored to the organization ledger"
              : " — anchor mismatch, the journal changed after the run sealed it")}
          .
        </p>
      )}
    </div>
  );
}
