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

/**
 * The per-workflow concurrency cap (Airflow's `max_active_runs`).
 *
 * Blank means uncapped, which is both the default and what every existing
 * workflow has — a cap is opt-in, because guessing one for a customer's system
 * is worse than leaving it off and letting them set it deliberately.
 */
export function ConcurrencyCap({
  workflowId,
  value,
}: {
  workflowId: string;
  value: number | null;
}) {
  const router = useRouter();
  const [draft, setDraft] = useState(value === null ? "" : String(value));
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function save(next: string) {
    if (next === (value === null ? "" : String(value))) return;
    setSaving(true);
    setError(null);
    try {
      const res = await fetch(`/api/workflows/${workflowId}`, {
        method: "PATCH",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ maxActiveRuns: next === "" ? null : Number(next) }),
      });
      const body = await res.json();
      if (!res.ok) throw new Error(body?.error ?? "failed");
      router.refresh();
    } catch (e) {
      setError((e as Error).message);
      setDraft(value === null ? "" : String(value));
    } finally {
      setSaving(false);
    }
  }

  return (
    <label className="flex items-center gap-1.5 text-xs text-[var(--color-muted)]">
      <span title="Maximum runs of this workflow in flight at once. Blank means no limit.">
        Max active
      </span>
      <input
        type="number"
        min={1}
        max={100}
        inputMode="numeric"
        placeholder="∞"
        value={draft}
        disabled={saving}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={(e) => void save(e.target.value)}
        className="w-14 rounded border border-[var(--color-border)] bg-transparent px-1.5 py-0.5 text-center text-xs"
        aria-label="Maximum active runs"
      />
      {error && <span className="text-[var(--color-danger)]">{error}</span>}
    </label>
  );
}

/**
 * Separation of duties: whoever triggers a run may not approve its gates.
 *
 * Opt-in, because an organization is created single-member on first sign-in and
 * there is no invite flow yet — switching this on in a one-person org means
 * nobody can approve anything and runs stop dead at their first gate. The label
 * says so rather than leaving it to be discovered.
 */
export function SeparateApprover({
  workflowId,
  value,
}: {
  workflowId: string;
  value: boolean;
}) {
  const router = useRouter();
  const [on, setOn] = useState(value);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function save(next: boolean) {
    setSaving(true);
    setError(null);
    setOn(next);
    try {
      const res = await fetch(`/api/workflows/${workflowId}`, {
        method: "PATCH",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ requireSeparateApprover: next }),
      });
      const body = await res.json();
      if (!res.ok) throw new Error(body?.error ?? "failed");
      router.refresh();
    } catch (e) {
      setError((e as Error).message);
      setOn(!next);
    } finally {
      setSaving(false);
    }
  }

  return (
    <label className="flex items-center gap-1.5 text-xs text-[var(--color-muted)]">
      <input
        type="checkbox"
        checked={on}
        disabled={saving}
        onChange={(e) => void save(e.target.checked)}
        aria-label="Require a separate approver"
      />
      <span title="Whoever starts a run cannot approve its gates — someone else in the organization must. Needs at least two people.">
        2nd approver
      </span>
      {error && <span className="text-[var(--color-danger)]">{error}</span>}
    </label>
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
