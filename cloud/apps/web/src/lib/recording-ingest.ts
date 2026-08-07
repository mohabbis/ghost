/**
 * Re-export: the implementation moved to `@ghost/core/recording/ingest` when
 * the worker's remote capture browser needed it too. Kept as a module so the
 * web app's existing import sites read naturally.
 */
export { ingestTrace, sanitizeFilename, MAX_TRACE_BYTES } from "@ghost/core/recording/ingest";
export type { IngestResult } from "@ghost/core/recording/ingest";
