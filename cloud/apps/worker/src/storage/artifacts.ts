import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { PutObjectCommand, S3Client } from "@aws-sdk/client-s3";

/**
 * Storage for run artifacts (per-step screenshots). Returns a stable key stored
 * on `RunStep.screenshotKey`; the web app resolves that key to a URL when
 * rendering the run timeline (S3 → presigned URL; disk → served route).
 *
 * S3-compatible when `S3_BUCKET` + creds are configured; otherwise a local-disk
 * fallback keeps dev self-contained.
 */
export interface ArtifactStore {
  put(key: string, body: Buffer, contentType: string): Promise<string>;
}

class DiskArtifactStore implements ArtifactStore {
  constructor(private readonly baseDir: string) {}

  async put(key: string, body: Buffer): Promise<string> {
    const full = join(this.baseDir, key);
    await mkdir(dirname(full), { recursive: true });
    await writeFile(full, body);
    return key;
  }
}

class S3ArtifactStore implements ArtifactStore {
  private readonly client: S3Client;
  constructor(
    private readonly bucket: string,
    region: string,
    endpoint: string | undefined,
  ) {
    this.client = new S3Client({
      region,
      ...(endpoint ? { endpoint, forcePathStyle: true } : {}),
    });
  }

  async put(key: string, body: Buffer, contentType: string): Promise<string> {
    await this.client.send(
      new PutObjectCommand({ Bucket: this.bucket, Key: key, Body: body, ContentType: contentType }),
    );
    return key;
  }
}

let store: ArtifactStore | undefined;

export function artifactStore(): ArtifactStore {
  if (store) return store;
  const bucket = process.env.S3_BUCKET;
  if (bucket && process.env.S3_ACCESS_KEY_ID && process.env.S3_SECRET_ACCESS_KEY) {
    store = new S3ArtifactStore(
      bucket,
      process.env.S3_REGION ?? "auto",
      process.env.S3_ENDPOINT || undefined,
    );
  } else {
    const dir = process.env.GHOST_ARTIFACT_DIR ?? resolve(process.cwd(), ".artifacts");
    store = new DiskArtifactStore(dir);
  }
  return store;
}

export function screenshotKey(runId: string, stepIndex: number): string {
  return `runs/${runId}/step-${stepIndex}.png`;
}
