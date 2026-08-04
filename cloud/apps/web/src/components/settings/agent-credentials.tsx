"use client";

import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";
import { Select } from "@/components/ui/input";

type Credential = {
  id: string;
  name: string;
  tokenHint: string;
  createdAt: string;
  lastUsedAt: string | null;
  expiresAt: string | null;
};

type OrgCredential = Credential & { user: { email: string | null; name: string | null } };

const EXPIRY_OPTIONS = [
  { label: "No expiry", value: "" },
  { label: "30 days", value: "30" },
  { label: "90 days", value: "90" },
  { label: "1 year", value: "365" },
] as const;

function expiryLabel(expiresAt: string | null): string {
  if (!expiresAt) return "No expiry";
  const date = new Date(expiresAt);
  const expired = date.getTime() <= Date.now();
  return `${expired ? "Expired" : "Expires"} ${date.toLocaleDateString()}`;
}

export function AgentCredentials({ isOrgAdmin }: { isOrgAdmin: boolean }) {
  const [credentials, setCredentials] = useState<Credential[]>([]);
  const [orgCredentials, setOrgCredentials] = useState<OrgCredential[] | null>(null);
  const [name, setName] = useState("Claude Code");
  const [expiresInDays, setExpiresInDays] = useState("");
  const [token, setToken] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [orgError, setOrgError] = useState<string | null>(null);

  async function refresh() {
    const res = await fetch("/api/settings/agent-credentials");
    const data = (await res.json()) as {
      credentials?: Credential[];
      error?: string;
    };
    if (!res.ok) throw new Error(data.error || "Could not load credentials");
    setCredentials(data.credentials ?? []);
  }

  async function refreshOrg() {
    const res = await fetch("/api/settings/agent-credentials/org");
    const data = (await res.json()) as {
      credentials?: OrgCredential[];
      error?: string;
    };
    if (!res.ok) throw new Error(data.error || "Could not load organization credentials");
    setOrgCredentials(data.credentials ?? []);
  }

  useEffect(() => {
    refresh().catch((err) =>
      setError(err instanceof Error ? err.message : "Could not load"),
    );
  }, []);

  useEffect(() => {
    if (!isOrgAdmin) return;
    refreshOrg().catch((err) =>
      setOrgError(err instanceof Error ? err.message : "Could not load"),
    );
  }, [isOrgAdmin]);

  async function createCredential() {
    setError(null);
    const res = await fetch("/api/settings/agent-credentials", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        name,
        expiresInDays: expiresInDays ? Number(expiresInDays) : undefined,
      }),
    });
    const data = (await res.json()) as { token?: string; error?: string };
    if (!res.ok || !data.token) {
      setError(data.error || "Could not create credential");
      return;
    }
    setToken(data.token);
    await refresh();
  }

  async function revoke(id: string, fromOrgList: boolean) {
    setError(null);
    setOrgError(null);
    const res = await fetch(`/api/settings/agent-credentials/${id}`, {
      method: "DELETE",
    });
    if (!res.ok) {
      const data = (await res.json()) as { error?: string };
      const message = data.error || "Could not revoke credential";
      if (fromOrgList) setOrgError(message);
      else setError(message);
      return;
    }
    await refresh();
    if (isOrgAdmin) await refreshOrg().catch(() => undefined);
  }

  return (
    <>
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
            <Select
              aria-label="Expiry"
              className="w-auto"
              onChange={(event) => setExpiresInDays(event.target.value)}
              value={expiresInDays}
            >
              {EXPIRY_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </Select>
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
                  <p className="text-xs text-[var(--color-muted)]">
                    {expiryLabel(credential.expiresAt)}
                  </p>
                </div>
                <Button variant="secondary" onClick={() => revoke(credential.id, false)}>
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

      {isOrgAdmin && (
        <Card>
          <CardHeader>
            <CardTitle>Organization credentials</CardTitle>
          </CardHeader>
          <CardBody className="space-y-4 text-sm">
            <p className="text-[var(--color-muted)]">
              Every active agent credential in this organization, not only
              yours. Visible because your role can manage the organization.
            </p>
            {orgError && <p className="text-red-500">{orgError}</p>}
            <div className="space-y-2">
              {(orgCredentials ?? []).map((credential) => (
                <div
                  className="flex items-center justify-between gap-3 rounded-md border border-[var(--color-border)] p-3"
                  key={credential.id}
                >
                  <div>
                    <p className="font-medium">{credential.name}</p>
                    <p className="text-xs text-[var(--color-muted)]">
                      {credential.user.email ?? credential.user.name ?? "unknown member"}
                    </p>
                    <p className="font-mono text-xs text-[var(--color-muted)]">
                      {credential.tokenHint}
                    </p>
                    <p className="text-xs text-[var(--color-muted)]">
                      {expiryLabel(credential.expiresAt)}
                    </p>
                  </div>
                  <Button variant="secondary" onClick={() => revoke(credential.id, true)}>
                    Revoke
                  </Button>
                </div>
              ))}
              {orgCredentials !== null && !orgCredentials.length && (
                <p className="text-[var(--color-muted)]">
                  No active agent credentials in this organization.
                </p>
              )}
            </div>
          </CardBody>
        </Card>
      )}
    </>
  );
}
