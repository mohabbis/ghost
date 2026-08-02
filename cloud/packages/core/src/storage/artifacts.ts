import { mkdir, writeFile, readFile, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { getSignedUrl } from "@aws-sdk/s3-request-presigner";
import {
  PutObjectCommand,
  GetObjectCommand,
  DeleteObjectCommand,
  DeleteObjectsCommand,
  ListObjectsV2Command,
  S3Client,
} from "@aws-sdk/client-s3";

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
  /**
   * Delete every object under a prefix.
   *
   * Separate from `delete` because object stores have no directories: on S3,
   * deleting the key `runs/<id>/session` removes an object with exactly that
   * name and leaves `runs/<id>/session/gate-2.bin` untouched. Purging session
   * credentials with `delete` therefore did nothing at all on S3, which is the
   * deployment where it matters most.
   */
  deletePrefix(prefix: string): Promise<void>;
  /**
   * A URL the browser can fetch this object from directly, or `null` when the
   * store cannot produce one and the caller must serve the bytes itself.
   *
   * Only ever call this for a key the serve route's allow-list has already
   * accepted. A signed URL is a bearer token for its lifetime: anyone holding
   * it can read the object without a session, so signing a key outside the
   * allow-list would hand out the encrypted session blob under
   * `runs/<id>/session/` to whoever saw the link.
   */
  signedUrl(key: string, ttlSeconds: number): Promise<string | null>;
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

  /** On disk a prefix is a real directory, so a recursive remove is enough. */
  async deletePrefix(prefix: string): Promise<void> {
    await rm(this.full(prefix), { force: true, recursive: true });
  }

  /** Nothing to sign: local files are reachable only through the app itself. */
  async signedUrl(): Promise<string | null> {
    return null;
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

  async deletePrefix(prefix: string): Promise<void> {
    // List then delete: S3 has no prefix delete, and the keys under a run's
    // session prefix are the encrypted browser credentials we most need gone.
    let token: string | undefined;
    do {
      const listed = await this.client.send(
        new ListObjectsV2Command({
          Bucket: this.bucket,
          Prefix: prefix.endsWith("/") ? prefix : `${prefix}/`,
          ContinuationToken: token,
        }),
      );
      const keys = (listed.Contents ?? [])
        .map((o) => o.Key)
        .filter((k): k is string => Boolean(k));
      if (keys.length > 0) {
        await this.client.send(
          new DeleteObjectsCommand({
            Bucket: this.bucket,
            Delete: { Objects: keys.map((Key) => ({ Key })) },
          }),
        );
      }
      token = listed.IsTruncated ? listed.NextContinuationToken : undefined;
    } while (token);
  }

  async signedUrl(key: string, ttlSeconds: number): Promise<string> {
    // Short-lived by default. The object is a screenshot of the customer's own
    // system mid-workflow, and the URL needs to survive only the moment between
    // the redirect and the browser fetching the image.
    return getSignedUrl(this.client, new GetObjectCommand({ Bucket: this.bucket, Key: key }), {
      expiresIn: ttlSeconds,
    });
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

// Key shapes and the "may this be served over HTTP?" rule live in core so the
// worker that writes them and the web route that gates them cannot drift.
export {
  screenshotKey,
  restoreScreenshotKey,
  compensationScreenshotKey,
  sessionStateKey,
  sessionPrefix,
} from "@ghost/core/artifacts";
