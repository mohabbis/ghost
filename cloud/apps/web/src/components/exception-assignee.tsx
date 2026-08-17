"use client";

import { useState, useTransition } from "react";
import { useRouter } from "next/navigation";

interface Member {
  id: string;
  name: string | null;
  email: string | null;
}

/**
 * Assign an open exception to a person.
 *
 * Deliberately a plain select rather than a modal or a drag target: this is the
 * control an ops lead uses a dozen times in a triage pass, and every extra click
 * is one they pay repeatedly. Assignment changes no run state and resumes
 * nothing, so it needs no confirmation step.
 *
 * The route is the authority on who may be assigned — it re-checks org
 * membership — so the list here is a convenience, not the security boundary.
 */
export function ExceptionAssignee({
  runId,
  assignee,
  members,
}: {
  runId: string;
  assignee: Member | null;
  members: Member[];
}) {
  const router = useRouter();
  const [pending, startTransition] = useTransition();
  const [error, setError] = useState<string | null>(null);
  const [value, setValue] = useState(assignee?.id ?? "");

  async function assign(next: string) {
    setError(null);
    setValue(next);
    const res = await fetch(`/api/runs/${runId}/incident`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        action: "assign",
        assigneeId: next === "" ? null : next,
      }),
    });
    if (!res.ok) {
      const body = (await res.json().catch(() => ({}))) as { error?: string };
      setError(body.error ?? "could not assign");
      // Put the control back to the truth, so it never shows an assignment the
      // server rejected.
      setValue(assignee?.id ?? "");
      return;
    }
    startTransition(() => router.refresh());
  }

  return (
    <div className="flex flex-wrap items-center gap-2 text-xs">
      <label
        className="text-[var(--color-muted)]"
        htmlFor={`assignee-${runId}`}
      >
        Owner
      </label>
      <select
        id={`assignee-${runId}`}
        value={value}
        disabled={pending}
        onChange={(e) => void assign(e.target.value)}
        className="rounded border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-xs"
      >
        <option value="">Unassigned</option>
        {members.map((m) => (
          <option key={m.id} value={m.id}>
            {m.name ?? m.email ?? m.id}
          </option>
        ))}
      </select>
      {error && <span className="text-[var(--color-danger)]">{error}</span>}
    </div>
  );
}
