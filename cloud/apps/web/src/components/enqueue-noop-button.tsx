"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";

export function EnqueueNoopButton() {
  const [state, setState] = useState<"idle" | "loading" | "done" | "error">("idle");
  const [detail, setDetail] = useState<string>("");

  async function enqueue() {
    setState("loading");
    setDetail("");
    try {
      const res = await fetch("/api/dev/enqueue-noop", { method: "POST" });
      const body = await res.json();
      if (!res.ok) throw new Error(body?.error ?? "failed");
      setState("done");
      setDetail(`Job ${body.jobId} queued — check the worker logs.`);
    } catch (err) {
      setState("error");
      setDetail(err instanceof Error ? err.message : "failed");
    }
  }

  return (
    <div className="flex items-center gap-3">
      <Button onClick={enqueue} disabled={state === "loading"} size="sm">
        {state === "loading" ? "Enqueuing…" : "Enqueue test job"}
      </Button>
      {detail && (
        <span
          className={
            state === "error"
              ? "text-sm text-[var(--color-danger)]"
              : "text-sm text-[var(--color-muted)]"
          }
        >
          {detail}
        </span>
      )}
    </div>
  );
}
