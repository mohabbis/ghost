"use client";

import { useState } from "react";
// The Node-free half of the contract. Importing from `recording/capture`
// instead would pull `node:crypto` into the client bundle and fail the build.
import { CAPTURE_VIEWPORT } from "@ghost/core/recording/capture-protocol";
import { Button } from "@/components/ui/button";
import { Card, CardBody } from "@/components/ui/card";
import { FieldLabel } from "@/components/ui/input";
import { CaptureSession } from "@/components/capture-session";

/**
 * Asks where to start, then hands off to the live session.
 *
 * Split from `CaptureSession` because the ticket is short-lived by design: it
 * is minted at the moment the user commits to starting, and the socket opens
 * immediately after. Minting one when the page loads would leave a credential
 * sitting in a tab nobody is looking at.
 */

interface Opened {
  socketUrl: string;
  ticket: string;
  startUrl: string;
}

export function CaptureLauncher() {
  const [startUrl, setStartUrl] = useState("");
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [session, setSession] = useState<Opened | null>(null);

  async function start() {
    setOpening(true);
    setError(null);
    try {
      const res = await fetch("/api/recordings/capture", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ startUrl }),
      });
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(body?.error ?? `could not start a session (${res.status})`);
      setSession({ socketUrl: body.socketUrl, ticket: body.ticket, startUrl: body.startUrl });
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setOpening(false);
    }
  }

  if (session) {
    return (
      <CaptureSession
        socketUrl={session.socketUrl}
        ticket={session.ticket}
        startUrl={session.startUrl}
        width={CAPTURE_VIEWPORT.width}
        height={CAPTURE_VIEWPORT.height}
      />
    );
  }

  return (
    <Card>
      <CardBody className="space-y-4">
        <div>
          <FieldLabel htmlFor="start-url">Start at</FieldLabel>
          <input
            id="start-url"
            value={startUrl}
            onChange={(e) => setStartUrl(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void start();
            }}
            placeholder="mail.google.com"
            spellCheck={false}
            className="mt-1 h-10 w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 text-sm"
          />
          <p className="mt-1 text-xs text-[var(--color-muted)]">
            The page the workflow begins on. You can navigate anywhere once it opens.
          </p>
        </div>

        {error && <p className="text-sm text-[var(--color-danger)]">{error}</p>}

        <div className="flex items-center gap-3">
          <Button onClick={() => void start()} disabled={opening}>
            {opening ? "Starting a browser…" : "Start recording"}
          </Button>
          <span className="text-xs text-[var(--color-muted)]">
            Nothing is saved until you press Save at the end.
          </span>
        </div>
      </CardBody>
    </Card>
  );
}
