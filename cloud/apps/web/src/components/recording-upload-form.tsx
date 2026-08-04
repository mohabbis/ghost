"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardBody } from "@/components/ui/card";
import { FieldLabel } from "@/components/ui/input";

export function RecordingUploadForm() {
  const router = useRouter();
  const [file, setFile] = useState<File | null>(null);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function upload() {
    if (!file) return;
    setUploading(true);
    setError(null);
    try {
      const form = new FormData();
      form.append("file", file);
      const res = await fetch("/api/recordings", { method: "POST", body: form });
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(body?.error ?? `upload failed (${res.status})`);
      router.push(`/recordings/${body.id}`);
      router.refresh();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setUploading(false);
    }
  }

  return (
    <Card>
      <CardBody className="space-y-4">
        <div>
          <FieldLabel htmlFor="trace-file">Trace file</FieldLabel>
          <input
            id="trace-file"
            type="file"
            accept=".json,.har,.zip,.txt"
            onChange={(e) => setFile(e.target.files?.[0] ?? null)}
            className="block w-full text-sm text-[var(--color-fg)] file:mr-3 file:rounded-lg file:border-0 file:bg-[var(--color-bg)] file:px-3 file:py-2 file:text-sm file:font-medium hover:file:bg-[var(--color-border)]/40"
          />
          <p className="mt-2 text-xs text-[var(--color-muted)]">
            A JSON event log, HAR export, or Playwright trace .zip of one demonstrated workflow.
          </p>
        </div>
        {error && <p className="text-sm text-[var(--color-danger)]">{error}</p>}
        <Button disabled={!file || uploading} onClick={upload}>
          {uploading ? "Uploading…" : "Upload"}
        </Button>
      </CardBody>
    </Card>
  );
}
