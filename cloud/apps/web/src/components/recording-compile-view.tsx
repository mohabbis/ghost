"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";
import { AlertTriangle, Loader2 } from "lucide-react";
import type { WorkflowStep } from "@ghost/core/schema/step";
import { Button } from "@/components/ui/button";
import { Card, CardBody } from "@/components/ui/card";
import { WorkflowEditor } from "@/components/workflow-editor";

export interface RecordingView {
  id: string;
  status: string;
  compileStatus: "NONE" | "RUNNING" | "READY" | "FAILED";
  rawTraceFilename: string | null;
  compiledSteps: unknown;
  compileNotes: string | null;
  compileError: string | null;
  workflowId: string | null;
}

const RUNNING = new Set(["RUNNING"]);

export function RecordingCompileView({ recordingId, initial }: { recordingId: string; initial: RecordingView }) {
  const [view, setView] = useState<RecordingView>(initial);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const res = await fetch(`/api/recordings/${recordingId}`, { cache: "no-store" });
    if (res.ok) setView((await res.json()) as RecordingView);
  }, [recordingId]);

  // Same reasoning as run-timeline.tsx: the stream route ends each
  // connection well inside the serverless time limit and relies on
  // EventSource's own reconnect, so closing here on a terminal
  // `compileStatus` is what actually stops the cycle.
  useEffect(() => {
    if (!RUNNING.has(view.compileStatus)) return;
    const source = new EventSource(`/api/recordings/${recordingId}/stream`);
    source.onmessage = (event) => {
      const data = JSON.parse(event.data as string) as RecordingView | { notFound: true };
      if ("notFound" in data) {
        source.close();
        return;
      }
      setView(data);
      if (!RUNNING.has(data.compileStatus)) source.close();
    };
    return () => source.close();
  }, [recordingId, view.compileStatus]);

  async function runAction(path: string) {
    setBusy(true);
    setActionError(null);
    try {
      const res = await fetch(`/api/recordings/${recordingId}${path}`, { method: "POST" });
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(body?.error ?? `request failed (${res.status})`);
      setView((v) => ({ ...v, compileStatus: "RUNNING", compileError: null }));
      await load();
    } catch (err) {
      setActionError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }

  if (view.workflowId) {
    return (
      <Card>
        <CardBody className="space-y-2">
          <p className="text-sm font-medium">Published</p>
          <p className="text-sm text-[var(--color-muted)]">
            This recording was compiled and published as a workflow.
          </p>
          <Link href={`/workflows/${view.workflowId}`} className="text-sm text-[var(--color-accent)] hover:underline">
            View the workflow →
          </Link>
        </CardBody>
      </Card>
    );
  }

  return (
    <div className="space-y-5">
      <Card>
        <CardBody className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <p className="text-sm font-medium">{view.rawTraceFilename ?? "recording trace"}</p>
            <p className="mt-1 text-sm text-[var(--color-muted)]">
              {statusLabel(view.compileStatus)}
            </p>
            {actionError && (
              <p className="mt-2 flex items-center gap-1.5 text-sm text-[var(--color-danger)]">
                <AlertTriangle className="h-3.5 w-3.5" /> {actionError}
              </p>
            )}
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {view.compileStatus === "NONE" && (
              <Button size="sm" disabled={busy} onClick={() => runAction("/compile")}>
                Compile
              </Button>
            )}
            {view.compileStatus === "RUNNING" && (
              <>
                <span className="inline-flex items-center gap-1.5 text-sm text-[var(--color-muted)]">
                  <Loader2 className="h-3.5 w-3.5 animate-spin" /> Compiling…
                </span>
                <Button size="sm" variant="secondary" disabled={busy} onClick={() => runAction("/cancel")}>
                  Cancel
                </Button>
              </>
            )}
            {view.compileStatus === "FAILED" && (
              <Button
                size="sm"
                disabled={busy}
                onClick={() =>
                  runAction(view.compileError?.includes('"incomplete"') ? "/continue" : "/compile")
                }
              >
                {view.compileError?.includes('"incomplete"') ? "Continue" : "Retry"}
              </Button>
            )}
            {view.compileStatus === "READY" && (
              <Button size="sm" variant="secondary" disabled={busy} onClick={() => runAction("/compile")}>
                Recompile
              </Button>
            )}
          </div>
        </CardBody>
      </Card>

      {view.compileStatus === "FAILED" && view.compileError && (
        <Card>
          <CardBody>
            <p className="text-sm text-[var(--color-danger)]">{view.compileError}</p>
          </CardBody>
        </Card>
      )}

      {/* Shown on READY *and* FAILED: on a failure these notes are usually
          the only account of what the compiler actually saw. */}
      {view.compileNotes && view.compileStatus !== "RUNNING" && (
        <Card>
          <CardBody>
            <p className="text-xs font-medium text-[var(--color-muted)]">Compiler notes</p>
            <p className="mt-1 whitespace-pre-wrap text-sm">{view.compileNotes}</p>
          </CardBody>
        </Card>
      )}

      {view.compileStatus === "READY" && (
        <>
          <div>
            <h2 className="text-sm font-semibold">Review proposed steps</h2>
            <p className="mt-1 text-sm text-[var(--color-muted)]">
              Edit anything before publishing — nothing here runs until you save it as a workflow.
            </p>
          </div>
          <WorkflowEditor
            initialSteps={view.compiledSteps as WorkflowStep[]}
            recordingId={recordingId}
          />
        </>
      )}
    </div>
  );
}

function statusLabel(status: RecordingView["compileStatus"]): string {
  switch (status) {
    case "NONE":
      return "Not compiled yet.";
    case "RUNNING":
      return "Compiling into editable steps…";
    case "READY":
      return "Proposed steps are ready for review.";
    case "FAILED":
      return "Compile failed.";
  }
}
