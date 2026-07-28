"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";

export function CreateDemoButton() {
  const router = useRouter();
  const [loading, setLoading] = useState(false);

  async function create() {
    setLoading(true);
    try {
      const res = await fetch("/api/workflows/demo", { method: "POST" });
      if (!res.ok) throw new Error("failed");
      router.refresh();
    } finally {
      setLoading(false);
    }
  }

  return (
    <Button onClick={create} disabled={loading} size="sm">
      {loading ? "Creating…" : "Create demo workflow"}
    </Button>
  );
}

export function RunButton({ workflowId }: { workflowId: string }) {
  const router = useRouter();
  const [loading, setLoading] = useState(false);

  async function run() {
    setLoading(true);
    try {
      const res = await fetch("/api/runs", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ workflowId }),
      });
      const body = await res.json();
      if (!res.ok) throw new Error(body?.error ?? "failed");
      router.push(`/runs/${body.runId}`);
    } catch {
      setLoading(false);
    }
  }

  return (
    <Button onClick={run} disabled={loading} variant="secondary" size="sm">
      {loading ? "Starting…" : "Run"}
    </Button>
  );
}
