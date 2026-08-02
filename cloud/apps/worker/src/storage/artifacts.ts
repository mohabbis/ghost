/**
 * Compatibility re-export.
 *
 * The store itself moved to `@ghost/core/storage/artifacts` so the web app can
 * read what the worker writes — previously it lived here and the web app's
 * serve route could only ever read local disk, which silently loses every
 * screenshot the moment web and worker are separate hosts.
 *
 * Kept as a re-export rather than rewriting every import, so this move is not
 * tangled up with the change that motivated it.
 */
export {
  type ArtifactStore,
  artifactStore,
  resetArtifactStore,
  screenshotKey,
  restoreScreenshotKey,
  compensationScreenshotKey,
  sessionStateKey,
  sessionPrefix,
} from "@ghost/core/storage/artifacts";
