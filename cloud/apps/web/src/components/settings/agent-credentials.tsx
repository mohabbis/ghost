"use client";

import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";

type Credential = {
  id: string;
  name: string;
  tokenHint: string;
  createdAt: string;
  lastUsedAt: string | null;
};

export function AgentCredentials() {
  const [credentials, setCredentials] = useState<Credential[]>([]);
  const [name, setName] = useState("Claude Code");
  const [token, setToken] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    const res = await fetch("/api/settings/agent-credentials");
    const data = (await res.json()) as {
      credentials?: Credential[];
      error?: string;
    };
    if (!res.ok) throw new Error(data.error || "Could not load credentials");
    setCredentials(data.credentials ?? []);
  }

  useEffect(() => {
    refresh().catch((err) =>
      setError(err instanceof Error ? err.message : "Could not load"),
    );
  }, []);

  async function createCredential() {
    setError(null);
    const res = await fetch("/api/settings/agent-credentials", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name }),
    });
    const data = (await res.json()) as { token?: string; error?: string };
    if (!res.ok || !data.token) {
      setError(data.error || "Could not create credential");
      return;
    }
    setToken(data.token);
    await refresh();
  }

  async function revoke(id: string) {
    setError(null);
    const res = await fetch(`/api/settings/agent-credentials/${id}`, {
      method: "DELETE",
    });
    if (!res.ok) {
      const data = (await res.json()) as { error?: string };
      setError(data.error || "Could not revoke credential");
      return;
    }
    await refresh();
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Claude Code and agent access</CardTitle>
      </CardHeader>
      <CardBody className="space-y-4 text-sm">
        <p className="text-[var(--color-muted)]">
          Create a revocable Ghost credential for the Claude Code plugin or
          another MCP client. Agents can propose runs, but approval remains in
          Ghost.
        </p>
        <div className="flex gap-2">
          <input
            aria-label="Credential name"
            className="min-w-0 flex-1 rounded-md border border-[var(--color-border)] bg-transparent px-3 py-2"
            maxLength={80}
            onChange={(event) => setName(event.target.value)}
            value={name}
          />
          <Button disabled={!name.trim()} onClick={createCredential}>
            Create credential
          </Button>
        </div>
        {token && (
          <div className="rounded-md border border-amber-500/40 bg-amber-500/10 p-3">
            <p className="font-medium">
              Copy this credential now. Ghost will not show it again.
            </p>
            <code className="mt-2 block break-all select-all text-xs">
              {token}
            </code>
          </div>
        )}
        {error && <p className="text-red-500">{error}</p>}
        <div className="space-y-2">
          {credentials.map((credential) => (
            <div
              className="flex items-center justify-between gap-3 rounded-md border border-[var(--color-border)] p-3"
              key={credential.id}
            >
              <div>
                <p className="font-medium">{credential.name}</p>
                <p className="font-mono text-xs text-[var(--color-muted)]">
                  {credential.tokenHint}
                </p>
              </div>
              <Button variant="secondary" onClick={() => revoke(credential.id)}>
                Revoke
              </Button>
            </div>
          ))}
          {!credentials.length && (
            <p className="text-[var(--color-muted)]">
              No active agent credentials.
            </p>
          )}
        </div>
      </CardBody>
    </Card>
  );
}
