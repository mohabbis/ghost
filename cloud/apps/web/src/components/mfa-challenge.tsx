"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardBody } from "@/components/ui/card";
import { FieldLabel, Input } from "@/components/ui/input";

export function MfaChallenge({ next }: { next: string }) {
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    setBusy(true);
    setError(null);
    try {
      const res = await fetch("/api/mfa/verify", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ code }),
      });
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error((body as { error?: string })?.error ?? "verification failed");
      // Full reload rather than a client navigation: the gate lives in
      // middleware, and only a real request carries the newly-set cookie
      // through it.
      window.location.href = next;
    } catch (err) {
      setError((err as Error).message);
      setBusy(false);
    }
  }

  return (
    <Card className="w-full">
      <CardBody className="space-y-4">
        <div>
          <h1 className="text-lg font-semibold">Two-factor verification</h1>
          <p className="mt-1 text-sm text-[var(--color-muted)]">
            Enter the 6-digit code from your authenticator app, or one of your recovery codes.
          </p>
        </div>
        <div>
          <FieldLabel htmlFor="mfa-code">Code</FieldLabel>
          <Input
            id="mfa-code"
            autoFocus
            autoComplete="one-time-code"
            inputMode="text"
            placeholder="123456"
            value={code}
            onChange={(e) => setCode(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && code.trim()) void submit();
            }}
          />
        </div>
        {error && <p className="text-sm text-[var(--color-danger)]">{error}</p>}
        <Button disabled={busy || !code.trim()} onClick={submit}>
          {busy ? "Verifying…" : "Verify"}
        </Button>
      </CardBody>
    </Card>
  );
}
