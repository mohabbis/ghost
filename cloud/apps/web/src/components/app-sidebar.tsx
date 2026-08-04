"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { LayoutDashboard, Workflow, Film, PlayCircle, ShieldCheck, Settings, Code2 } from "lucide-react";
import { cn } from "@/lib/cn";
import { SOURCE_URL } from "@/lib/source-url";

const NAV = [
  { href: "/dashboard", label: "Dashboard", icon: LayoutDashboard },
  { href: "/workflows", label: "Workflows", icon: Workflow },
  { href: "/recordings", label: "Recordings", icon: Film },
  { href: "/runs", label: "Runs", icon: PlayCircle },
  { href: "/audit", label: "Audit", icon: ShieldCheck },
  { href: "/settings", label: "Settings", icon: Settings },
];

export function AppSidebar({ orgName }: { orgName: string }) {
  const pathname = usePathname();
  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-[var(--color-border)] bg-[var(--color-surface)]">
      <div className="flex h-14 items-center gap-2 border-b border-[var(--color-border)] px-4">
        <span className="grid h-6 w-6 place-items-center rounded-md bg-[var(--color-accent)] text-xs font-bold text-[var(--color-accent-fg)]">
          G
        </span>
        <span className="text-sm font-semibold">Ghost</span>
      </div>

      <nav className="flex-1 space-y-1 p-2">
        {NAV.map(({ href, label, icon: Icon }) => {
          const active = pathname === href || pathname.startsWith(href + "/");
          return (
            <Link
              key={href}
              href={href}
              className={cn(
                "flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm transition-colors",
                active
                  ? "bg-[var(--color-bg)] font-medium text-[var(--color-fg)]"
                  : "text-[var(--color-muted)] hover:bg-[var(--color-bg)]/60 hover:text-[var(--color-fg)]",
              )}
            >
              <Icon className="h-4 w-4" />
              {label}
            </Link>
          );
        })}
      </nav>

      <div className="space-y-1.5 border-t border-[var(--color-border)] p-3">
        <div className="truncate text-xs text-[var(--color-muted)]" title={orgName}>
          {orgName}
        </div>
        {/*
          AGPL-3.0 section 13: users interacting with Ghost over a network must be
          offered its Corresponding Source. A LICENSE file in the repo does not
          reach them, so the running app has to carry the link itself.
        */}
        <a
          href={SOURCE_URL}
          target="_blank"
          rel="noreferrer"
          className="flex items-center gap-1.5 text-xs text-[var(--color-muted)] hover:text-[var(--color-fg)]"
        >
          <Code2 className="h-3.5 w-3.5" />
          AGPL-3.0 · Source
        </a>
      </div>
    </aside>
  );
}
