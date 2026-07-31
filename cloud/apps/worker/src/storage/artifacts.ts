import { mkdir, writeFile, readFile, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { PutObjectCommand, GetObjectCommand, DeleteObjectCommand, S3Client } from "@aws-sdk/client-s3";

/**
 * Storage for run artifacts. Returns a stable key stored on the run row; the
 * web app resolves screenshot keys to URLs when rendering the run timeline
 * (S3 → presigned URL; disk → served route).
 *
 * S3-compatible when `S3_BUCKET` + creds are configured; otherwise a local-disk
 * fallback keeps dev self-contained.
 *
 * Two namespaces with very different sensitivity share this store:
 *
 *   `runs/<id>/step-<n>.png`          screenshots — served to the browser
 *   `runs/<id>/session/gate-<n>.bin`  ENCRYPTED storageState — never served
 *
 * The web app's artifact route allow-lists the first and hard-denies the
 * second. See apps/web/src/app/api/artifacts/[...key]/route.ts.
 */
export interface ArtifactStore {
  put(key: string, body: Buffer, contentType: string): Promise<string>;
  get(key: string): Promise<Buffer>;
  delete(key: string): Promise<void>;
}

class DiskArtifactStore implements ArtifactStore {
  constructor(private readonly baseDir: string) {}

  private full(key: string): string {
    const full = resolve(join(this.baseDir, key));
    // Containment check: a key must never escape the artifact root.
    const root = resolve(this.baseDir);
    if (full !== root && !full.startsWith(root + "/")) {
      throw new Error(`artifact key escapes the artifact root: ${key}`);
    }
    return full;
  }

  async put(key: string, body: Buffer): Promise<string> {
    const full = this.full(key);
    await mkdir(dirname(full), { recursive: true });
    await writeFile(full, body);
    return key;
  }

  async get(key: string): Promise<Buffer> {
    return readFile(this.full(key));
  }

  async delete(key: string): Promise<void> {
    await rm(this.full(key), { force: true, recursive: true });
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

  async get(key: string): Promise<Buffer> {
    const res = await this.client.send(
      new GetObjectCommand({ Bucket: this.bucket, Key: key }),
    );
    if (!res.Body) throw new Error(`artifact ${key} has no body`);
    return Buffer.from(await res.Body.transformToByteArray());
  }

  async delete(key: string): Promise<void> {
    await this.client.send(new DeleteObjectCommand({ Bucket: this.bucket, Key: key }));
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

/** Test seam: drop the memoized store so env changes take effect. */
export function resetArtifactStore(): void {
  store = undefined;
}

export function screenshotKey(runId: string, stepIndex: number): string {
  return `runs/${runId}/step-${stepIndex}.png`;
}

/** Screenshot taken immediately after a restore, so a human can see the page. */
export function restoreScreenshotKey(runId: string, stepIndex: number): string {
  return `runs/${runId}/restore-${stepIndex}.png`;
}

/**
 * Encrypted browser session state captured at a halt.
 *
 * Deliberately under a `session/` segment the web artifact route refuses to
 * serve — this blob is a live credential, not an artifact to look at.
 */
export function sessionStateKey(runId: string, stepIndex: number): string {
  return `runs/${runId}/session/gate-${stepIndex}.bin`;
}

/** Prefix holding every session blob for a run, purged when the run ends. */
export function sessionPrefix(runId: string): string {
  return `runs/${runId}/session`;
}
