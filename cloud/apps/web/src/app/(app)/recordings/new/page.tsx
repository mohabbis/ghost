import Link from "next/link";
import { RecordingUploadForm } from "@/components/recording-upload-form";

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

      <RecordingUploadForm />
    </div>
  );
}
