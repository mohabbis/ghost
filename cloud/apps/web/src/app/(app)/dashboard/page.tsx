import { Card, CardBody, CardHeader, CardTitle } from "@/components/ui/card";

export const dynamic = "force-dynamic";

const CAPABILITIES = [
  "Record a browser workflow",
  "Convert it into editable steps",
  "Replay across browser / API actions",
  "Approve sensitive actions before they execute",
  "Verify the outcome and log every run",
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
          <CardTitle>What Ghost does</CardTitle>
        </CardHeader>
        <CardBody className="space-y-2">
          {CAPABILITIES.map((label) => (
            <div key={label} className="text-sm">
              {label}
            </div>
          ))}
        </CardBody>
      </Card>
    </div>
  );
}
