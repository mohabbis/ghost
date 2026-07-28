import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";
import { EnqueueNoopButton } from "@/components/enqueue-noop-button";

export const dynamic = "force-dynamic";

const PILLARS = [
  { n: 1, label: "Record a browser workflow", status: "Phase 2" },
  { n: 2, label: "Convert it into editable steps", status: "Phase 2" },
  { n: 3, label: "Replay across browser / API actions", status: "Phase 1" },
  { n: 4, label: "Approve sensitive actions", status: "Phase 1" },
  { n: 5, label: "Log every run with verification", status: "Phase 1" },
];

export default function DashboardPage() {
  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <div>
        <h1 className="text-xl font-semibold">Dashboard</h1>
        <p className="mt-1 text-sm text-[var(--color-muted)]">
          Teach Ghost a workflow once — it executes it reliably, pauses for approval on
          sensitive steps, verifies the outcome, and logs what changed.
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>MVP capabilities</CardTitle>
        </CardHeader>
        <CardBody className="space-y-2">
          {PILLARS.map((p) => (
            <div key={p.n} className="flex items-center justify-between text-sm">
              <span>
                <span className="mr-2 text-[var(--color-muted)]">{p.n}.</span>
                {p.label}
              </span>
              <span className="rounded-md border border-[var(--color-border)] px-2 py-0.5 text-xs text-[var(--color-muted)]">
                {p.status}
              </span>
            </div>
          ))}
        </CardBody>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Wiring check</CardTitle>
        </CardHeader>
        <CardBody className="space-y-3">
          <p className="text-sm text-[var(--color-muted)]">
            Enqueue a no-op job to confirm the web → Redis → worker path is live.
          </p>
          <EnqueueNoopButton />
        </CardBody>
      </Card>
    </div>
  );
}
