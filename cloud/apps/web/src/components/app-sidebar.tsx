"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { LayoutDashboard, Workflow, PlayCircle, Settings } from "lucide-react";
import { cn } from "@/lib/cn";

const NAV = [
  { href: "/dashboard", label: "Dashboard", icon: LayoutDashboard },
  { href: "/workflows", label: "Workflows", icon: Workflow },
  { href: "/runs", label: "Runs", icon: PlayCircle },
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

      <div className="border-t border-[var(--color-border)] p-3">
        <div className="truncate text-xs text-[var(--color-muted)]" title={orgName}>
          {orgName}
        </div>
      </div>
    </aside>
  );
}
