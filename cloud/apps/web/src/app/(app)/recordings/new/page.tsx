import Link from "next/link";
import { RecordingUploadForm } from "@/components/recording-upload-form";
import { Button } from "@/components/ui/button";
import { Card, CardBody } from "@/components/ui/card";

export default function NewRecordingPage() {
  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <div>
        <Link href="/recordings" className="text-sm text-[var(--color-muted)] hover:underline">
          ← Recordings
        </Link>
        <h1 className="mt-2 text-xl font-semibold">New recording</h1>
        <p className="mt-1 text-sm text-[var(--color-muted)]">
          Upload the trace of one demonstrated workflow. Ghost compiles it into editable steps you
          review before anything runs.
        </p>
      </div>

      {/* The path most people should take. Uploading a trace file assumes you
          already have one, which assumes a recorder you had to install — a fine
          option for someone who wants it, and the wrong thing to lead with. */}
      <Card>
        <CardBody className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <p className="text-sm font-medium">Record in a browser instead</p>
            <p className="mt-1 text-sm text-[var(--color-muted)]">
              Do the task once in a browser Ghost runs for you. Nothing to install.
            </p>
          </div>
          <Link href="/recordings/capture">
            <Button>Record a workflow</Button>
          </Link>
        </CardBody>
      </Card>

      <RecordingUploadForm />
    </div>
  );
}
