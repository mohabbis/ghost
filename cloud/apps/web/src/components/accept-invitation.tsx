"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardBody } from "@/components/ui/card";

interface Accepted {
  orgName: string;
  role: string;
  alreadyMember: boolean;
  reauthRequired: boolean;
}

export function AcceptInvitation({ token }: { token: string }) {
  const router = useRouter();
  const [state, setState] = useState<"idle" | "working" | "done">("idle");
  const [accepted, setAccepted] = useState<Accepted | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function accept() {
    setState("working");
    setError(null);
    try {
      const res = await fetch("/api/invitations/accept", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ token }),
      });
      if (res.status === 401) {
        // Not signed in. Bounce through sign-in and come straight back to
        // this same URL so the token is not lost.
        router.push(`/signin?from=${encodeURIComponent(`/invitations/${token}`)}`);
        return;
      }
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error((body as { error?: string })?.error ?? "could not accept");
      setAccepted(body as Accepted);
      setState("done");
    } catch (err) {
      setError((err as Error).message);
      setState("idle");
    }
  }

  if (state === "done" && accepted) {
    return (
      <Card className="w-full">
        <CardBody className="space-y-3">
          <p className="text-sm font-medium">
            {accepted.alreadyMember
              ? `You were already in ${accepted.orgName}.`
              : `You've joined ${accepted.orgName} as ${accepted.role}.`}
          </p>
          {accepted.reauthRequired && (
            <p className="text-sm text-[var(--color-muted)]">
              Sign out and back in to switch to this workspace — Ghost picks your workspace at
              sign-in and cannot switch it mid-session yet.
            </p>
          )}
          <Button onClick={() => router.push("/dashboard")}>Go to Ghost</Button>
        </CardBody>
      </Card>
    );
  }

  return (
    <Card className="w-full">
      <CardBody className="space-y-4">
        <div>
          <h1 className="text-lg font-semibold">You&apos;ve been invited to Ghost</h1>
          <p className="mt-1 text-sm text-[var(--color-muted)]">
            Accept to join the workspace. You&apos;ll need to be signed in as the address the
            invitation was sent to.
          </p>
        </div>
        {error && <p className="text-sm text-[var(--color-danger)]">{error}</p>}
        <Button disabled={state === "working"} onClick={accept}>
          {state === "working" ? "Accepting…" : "Accept invitation"}
        </Button>
      </CardBody>
    </Card>
  );
}
