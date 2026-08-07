import Link from "next/link";
import { CaptureLauncher } from "@/components/capture-launcher";

export const metadata = { title: "Record a workflow" };

export default function CapturePage() {
  return (
    <div className="mx-auto max-w-5xl space-y-6">
      <div>
        <Link href="/recordings" className="text-sm text-[var(--color-muted)] hover:underline">
          ← Recordings
        </Link>
        <h1 className="mt-2 text-xl font-semibold">Record a workflow</h1>
        <p className="mt-1 text-sm text-[var(--color-muted)]">
          Do the task once in the browser below. Ghost watches what you click and type, and turns
          it into steps you review and edit before anything runs.
        </p>
      </div>

      <CaptureLauncher />
    </div>
  );
}
