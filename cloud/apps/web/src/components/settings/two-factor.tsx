"use client";

import { useCallback, useEffect, useState } from "react";
import { ShieldCheck } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";
import { FieldLabel, Input } from "@/components/ui/input";

interface Status {
  available: boolean;
  enabled: boolean;
  enrolling: boolean;
  recoveryCodesRemaining: number;
}

export function TwoFactor() {
  const [status, setStatus] = useState<Status | null>(null);
  const [enrolment, setEnrolment] = useState<{ secret: string; uri: string } | null>(null);
  const [recoveryCodes, setRecoveryCodes] = useState<string[] | null>(null);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const res = await fetch("/api/settings/mfa", { cache: "no-store" });
    if (res.ok) setStatus((await res.json()) as Status);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function call(method: string, body?: unknown) {
    setBusy(true);
    setError(null);
    try {
      const res = await fetch("/api/settings/mfa", {
        method,
        headers: body ? { "content-type": "application/json" } : undefined,
        body: body ? JSON.stringify(body) : undefined,
      });
      const parsed = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error((parsed as { error?: string })?.error ?? `failed (${res.status})`);
      return parsed as Record<string, unknown>;
    } finally {
      setBusy(false);
    }
  }

  if (!status) return null;

  if (!status.available) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>Two-factor authentication</CardTitle>
        </CardHeader>
        <CardBody>
          <p className="text-sm text-[var(--color-muted)]">
            Unavailable on this deployment: <code className="text-xs">GHOST_MFA_KEY</code> is not
            set, and Ghost will not store two-factor secrets unencrypted.
          </p>
        </CardBody>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader className="flex items-center justify-between">
        <CardTitle>Two-factor authentication</CardTitle>
        {status.enabled && (
          <span className="inline-flex items-center gap-1.5 text-xs text-[var(--color-muted)]">
            <ShieldCheck className="h-3.5 w-3.5" /> On
          </span>
        )}
      </CardHeader>
      <CardBody className="space-y-4">
        {error && <p className="text-sm text-[var(--color-danger)]">{error}</p>}

        {recoveryCodes && (
          <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-3">
            <p className="text-xs font-medium">Save your recovery codes</p>
            <p className="mt-1 text-xs text-[var(--color-muted)]">
              Each works once if you lose your authenticator. Shown only now — Ghost stores only
              their hashes and cannot show them again.
            </p>
            <div className="mt-2 grid grid-cols-2 gap-1 font-mono text-xs">
              {recoveryCodes.map((c) => (
                <span key={c}>{c}</span>
              ))}
            </div>
            <Button
              size="sm"
              variant="secondary"
              className="mt-3"
              onClick={() => {
                void navigator.clipboard.writeText(recoveryCodes.join("\n"));
              }}
            >
              Copy all
            </Button>
          </div>
        )}

        {!status.enabled && !enrolment && (
          <>
            <p className="text-sm text-[var(--color-muted)]">
              Require a code from an authenticator app in addition to your sign-in.
            </p>
            <Button
              disabled={busy}
              onClick={async () => {
                try {
                  const r = await call("POST");
                  setEnrolment({ secret: r.secret as string, uri: r.uri as string });
                } catch (err) {
                  setError((err as Error).message);
                }
              }}
            >
              Set up
            </Button>
          </>
        )}

        {!status.enabled && enrolment && (
          <div className="space-y-3">
            <p className="text-sm">
              Add this to your authenticator app, then enter the code it shows.
            </p>
            <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-3">
              <p className="text-xs text-[var(--color-muted)]">Setup key</p>
              <code className="mt-1 block break-all font-mono text-xs">{enrolment.secret}</code>
              <p className="mt-2 text-xs text-[var(--color-muted)]">
                Or open this URI on the device with your authenticator:
              </p>
              <code className="mt-1 block break-all font-mono text-[10px] text-[var(--color-muted)]">
                {enrolment.uri}
              </code>
            </div>
            <div>
              <FieldLabel htmlFor="mfa-confirm">Code from the app</FieldLabel>
              <Input
                id="mfa-confirm"
                placeholder="123456"
                value={code}
                onChange={(e) => setCode(e.target.value)}
              />
            </div>
            <Button
              disabled={busy || !code.trim()}
              onClick={async () => {
                try {
                  const r = await call("PUT", { code });
                  setRecoveryCodes(r.recoveryCodes as string[]);
                  setEnrolment(null);
                  setCode("");
                  await load();
                } catch (err) {
                  setError((err as Error).message);
                }
              }}
            >
              Turn on
            </Button>
          </div>
        )}

        {status.enabled && (
          <div className="space-y-3">
            <p className="text-sm text-[var(--color-muted)]">
              {status.recoveryCodesRemaining} recovery code
              {status.recoveryCodesRemaining === 1 ? "" : "s"} remaining.
            </p>
            <div>
              <FieldLabel htmlFor="mfa-disable">Enter a current code to turn off</FieldLabel>
              <Input
                id="mfa-disable"
                placeholder="123456"
                value={code}
                onChange={(e) => setCode(e.target.value)}
              />
            </div>
            <Button
              variant="danger"
              disabled={busy || !code.trim()}
              onClick={async () => {
                try {
                  await call("DELETE", { code });
                  setCode("");
                  setRecoveryCodes(null);
                  await load();
                } catch (err) {
                  setError((err as Error).message);
                }
              }}
            >
              Turn off
            </Button>
          </div>
        )}
      </CardBody>
    </Card>
  );
}
