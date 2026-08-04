"use client";

import { useCallback, useEffect, useState } from "react";
import { Copy, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";
import { FieldLabel, Input, Select } from "@/components/ui/input";

interface Member {
  userId: string;
  email: string | null;
  name: string | null;
  role: "OWNER" | "ADMIN" | "MEMBER";
  isSelf: boolean;
}
interface Invitation {
  id: string;
  email: string;
  role: string;
  expiresAt: string;
}

const ROLE_HELP: Record<string, string> = {
  OWNER: "Full control, including managing owners.",
  ADMIN: "Manage members, credentials and settings.",
  MEMBER: "Author and run workflows; approve gates.",
};

export function Members({ isOrgAdmin }: { isOrgAdmin: boolean }) {
  const [members, setMembers] = useState<Member[]>([]);
  const [invitations, setInvitations] = useState<Invitation[]>([]);
  const [email, setEmail] = useState("");
  const [role, setRole] = useState("MEMBER");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [issuedLink, setIssuedLink] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const load = useCallback(async () => {
    const [m, i] = await Promise.all([
      fetch("/api/settings/members", { cache: "no-store" }),
      isOrgAdmin
        ? fetch("/api/settings/members/invitations", { cache: "no-store" })
        : Promise.resolve(null),
    ]);
    if (m.ok) setMembers(((await m.json()) as { members: Member[] }).members);
    if (i?.ok) setInvitations(((await i.json()) as { invitations: Invitation[] }).invitations);
  }, [isOrgAdmin]);

  useEffect(() => {
    void load();
  }, [load]);

  async function act(fn: () => Promise<Response>, onOk?: (body: unknown) => void) {
    setBusy(true);
    setError(null);
    try {
      const res = await fn();
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error((body as { error?: string })?.error ?? `failed (${res.status})`);
      onOk?.(body);
      await load();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card>
      <CardHeader className="flex items-center justify-between">
        <CardTitle>Members</CardTitle>
      </CardHeader>
      <CardBody className="space-y-5">
        {error && <p className="text-sm text-[var(--color-danger)]">{error}</p>}

        <div className="space-y-2">
          {members.map((m) => (
            <div
              key={m.userId}
              className="flex items-center justify-between gap-3 rounded-lg border border-[var(--color-border)] px-3 py-2"
            >
              <div className="min-w-0">
                <div className="truncate text-sm">
                  {m.email ?? m.name ?? m.userId}
                  {m.isSelf && <span className="ml-2 text-xs text-[var(--color-muted)]">you</span>}
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {isOrgAdmin ? (
                  <Select
                    className="h-8 w-32"
                    value={m.role}
                    disabled={busy}
                    onChange={(e) =>
                      act(() =>
                        fetch(`/api/settings/members/${m.userId}`, {
                          method: "PATCH",
                          headers: { "content-type": "application/json" },
                          body: JSON.stringify({ role: e.target.value }),
                        }),
                      )
                    }
                  >
                    {Object.keys(ROLE_HELP).map((r) => (
                      <option key={r} value={r}>
                        {r}
                      </option>
                    ))}
                  </Select>
                ) : (
                  <span className="text-xs text-[var(--color-muted)]">{m.role}</span>
                )}
                {isOrgAdmin && (
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={busy}
                    onClick={() =>
                      act(() =>
                        fetch(`/api/settings/members/${m.userId}`, { method: "DELETE" }),
                      )
                    }
                  >
                    Remove
                  </Button>
                )}
              </div>
            </div>
          ))}
        </div>

        {isOrgAdmin && (
          <>
            <div className="border-t border-[var(--color-border)] pt-4">
              <FieldLabel htmlFor="invite-email">Invite someone</FieldLabel>
              <div className="flex gap-2">
                <Input
                  id="invite-email"
                  type="email"
                  placeholder="colleague@company.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                />
                <Select className="w-32" value={role} onChange={(e) => setRole(e.target.value)}>
                  {Object.keys(ROLE_HELP).map((r) => (
                    <option key={r} value={r}>
                      {r}
                    </option>
                  ))}
                </Select>
                <Button
                  disabled={busy || !email.trim()}
                  onClick={() =>
                    act(
                      () =>
                        fetch("/api/settings/members/invitations", {
                          method: "POST",
                          headers: { "content-type": "application/json" },
                          body: JSON.stringify({ email, role }),
                        }),
                      (body) => {
                        setIssuedLink((body as { acceptUrl: string }).acceptUrl);
                        setEmail("");
                        setCopied(false);
                      },
                    )
                  }
                >
                  {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : "Invite"}
                </Button>
              </div>
              <p className="mt-2 text-xs text-[var(--color-muted)]">{ROLE_HELP[role]}</p>
            </div>

            {issuedLink && (
              <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-3">
                <p className="text-xs font-medium">Send this link to the person you invited</p>
                <p className="mt-1 text-xs text-[var(--color-muted)]">
                  Ghost does not send the email yet. This is shown once — it cannot be recovered.
                </p>
                <div className="mt-2 flex items-center gap-2">
                  <code className="min-w-0 flex-1 truncate rounded bg-[var(--color-surface)] px-2 py-1 text-xs">
                    {issuedLink}
                  </code>
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => {
                      void navigator.clipboard.writeText(issuedLink);
                      setCopied(true);
                    }}
                  >
                    <Copy className="h-3.5 w-3.5" />
                    {copied ? "Copied" : "Copy"}
                  </Button>
                </div>
              </div>
            )}

            {invitations.length > 0 && (
              <div className="space-y-2 border-t border-[var(--color-border)] pt-4">
                <p className="text-xs font-medium text-[var(--color-muted)]">Pending invitations</p>
                {invitations.map((inv) => (
                  <div
                    key={inv.id}
                    className="flex items-center justify-between gap-3 rounded-lg border border-[var(--color-border)] px-3 py-2"
                  >
                    <div className="min-w-0 text-sm">
                      <span className="truncate">{inv.email}</span>
                      <span className="ml-2 text-xs text-[var(--color-muted)]">{inv.role}</span>
                    </div>
                    <Button
                      size="sm"
                      variant="secondary"
                      disabled={busy}
                      onClick={() =>
                        act(() =>
                          fetch(`/api/settings/members/invitations/${inv.id}`, {
                            method: "DELETE",
                          }),
                        )
                      }
                    >
                      Revoke
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </>
        )}
      </CardBody>
    </Card>
  );
}
