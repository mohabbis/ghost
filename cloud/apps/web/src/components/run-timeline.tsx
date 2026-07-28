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
}
interface ApprovalView {
  stepIndex: number;
  status: string;
  reason: string;
}
interface RunView {
  id: string;
  status: string;
  error: string | null;
  workflowName: string;
  steps: StepView[];
  approvals: ApprovalView[];
}

const TERMINAL = new Set(["SUCCEEDED", "FAILED", "CANCELED"]);

function statusColor(status: string): string {
  if (status === "SUCCEEDED") return "text-[var(--color-success)]";
  if (status === "FAILED" || status === "CANCELED") return "text-[var(--color-danger)]";
  if (status === "AWAITING_APPROVAL") return "text-[var(--color-warning)]";
  return "text-[var(--color-muted)]";
}

export function RunTimeline({ runId }: { runId: string }) {
  const [run, setRun] = useState<RunView | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    const res = await fetch(`/api/runs/${runId}`, { cache: "no-store" });
    if (res.ok) setRun((await res.json()) as RunView);
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

  async function resolve(stepIndex: number, decision: "approve" | "reject") {
    setBusy(true);
    try {
      await fetch(`/api/runs/${runId}/approvals/${stepIndex}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ decision }),
      });
      await load();
    } finally {
      setBusy(false);
    }
  }

  if (!run) return <p className="text-sm text-[var(--color-muted)]">Loading run…</p>;

  const pending = run.approvals.find((a) => a.status === "PENDING");

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">{run.workflowName}</h1>
          <p className="mt-1 font-mono text-xs text-[var(--color-muted)]">{run.id}</p>
        </div>
        <span className={`text-sm font-medium ${statusColor(run.status)}`}>{run.status}</span>
      </div>

      {run.error && <p className="text-sm text-[var(--color-danger)]">{run.error}</p>}

      {pending && (
        <Card>
          <CardBody className="flex items-center justify-between gap-4">
            <div className="text-sm">
              <span className="font-medium">Approval required</span> — {pending.reason}
            </div>
            <div className="flex gap-2">
              <Button size="sm" disabled={busy} onClick={() => resolve(pending.stepIndex, "approve")}>
                Approve
              </Button>
              <Button
                size="sm"
                variant="danger"
                disabled={busy}
                onClick={() => resolve(pending.stepIndex, "reject")}
              >
                Reject
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
                    <span className={`text-xs ${statusColor(s.status)}`}>{s.status}</span>
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
    </div>
  );
}
