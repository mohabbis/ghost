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
}
interface RunView {
  id: string;
  status: string;
  error: string | null;
  workflowName: string;
  cursor: number;
  steps: StepView[];
  approvals: ApprovalView[];
}
interface ChainView {
  org: { intact: boolean; count: number };
  run?: { intact: boolean; count: number; anchored: boolean; anchorMatches: boolean | null };
}

const TERMINAL = new Set(["SUCCEEDED", "FAILED", "CANCELED"]);
const STOPPABLE = new Set(["QUEUED", "RUNNING", "AWAITING_APPROVAL", "INCIDENT"]);

function statusColor(status: string): string {
  if (status === "SUCCEEDED") return "text-[var(--color-success)]";
  if (status === "FAILED" || status === "CANCELED") return "text-[var(--color-danger)]";
  // An indeterminate outcome is not a failure and must not read like one — it
  // means nobody knows whether the action took effect.
  if (status === "UNKNOWN" || status === "INCIDENT") return "text-[var(--color-warning)]";
  if (status === "AWAITING_APPROVAL") return "text-[var(--color-warning)]";
  return "text-[var(--color-muted)]";
}

export function RunTimeline({ runId }: { runId: string }) {
  const [run, setRun] = useState<RunView | null>(null);
  const [chain, setChain] = useState<ChainView | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    const res = await fetch(`/api/runs/${runId}`, { cache: "no-store" });
    if (res.ok) setRun((await res.json()) as RunView);
  }, [runId]);

  const loadChain = useCallback(async () => {
    const res = await fetch(`/api/audit/verify?runId=${runId}`, { cache: "no-store" });
    if (res.ok) setChain((await res.json()) as ChainView);
  }, [runId]);

  useEffect(() => {
    let active = true;
    void load();
    const timer = setInterval(() => {
      if (!active) return;
      setRun((cur) => {
        if (cur && TERMINAL.has(cur.status)) return cur;
        void load();
        return cur;
      });
    }, 1500);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [load]);

  // Verify the chain once the run stops changing.
  useEffect(() => {
    if (run && TERMINAL.has(run.status)) void loadChain();
  }, [run, loadChain]);

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
              Stop after current step
            </Button>
          )}
        </div>
      </div>

      {run.error && <p className="text-sm text-[var(--color-danger)]">{run.error}</p>}
      {notice && <p className="text-sm text-[var(--color-danger)]">{notice}</p>}

      {pending && (
        <Card>
          <CardBody className="flex items-center justify-between gap-4">
            <div className="text-sm">
              <span className="font-medium">Approval required</span> — {pending.reason}
              {pending.expiresAt && (
                <span className="ml-2 text-xs text-[var(--color-muted)]">
                  expires {new Date(pending.expiresAt).toLocaleString()}
                </span>
              )}
            </div>
            <div className="flex gap-2">
              <Button
                size="sm"
                disabled={busy}
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
            {incidentStep?.status === "UNKNOWN" && (
              <p className="text-xs text-[var(--color-muted)]">
                This step started but never reported an outcome, and its effect cannot safely be
                repeated. Check the target system before retrying — Ghost will not guess whether it
                took effect.
              </p>
            )}
            <div className="flex gap-2">
              <Button
                size="sm"
                disabled={busy}
                onClick={() => post(`/api/runs/${run.id}/incident`, { action: "retry" })}
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
