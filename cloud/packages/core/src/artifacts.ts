/**
 * Run artifact key namespace — the single definition shared by the worker that
 * writes keys and the web route that serves them.
 *
 * Two kinds of thing live under `runs/<id>/`, with very different sensitivity:
 *
 *   `runs/<id>/step-<n>.png`          screenshots — safe to show a user
 *   `runs/<id>/restore-<n>.png`       post-restore evidence — safe to show
 *   `runs/<id>/session/gate-<n>.bin`  ENCRYPTED storageState — never served
 *
 * The session blob holds live session cookies for the customer's systems. It is
 * encrypted at rest, but it must also never be reachable over HTTP, so serving
 * is an allow-list rather than a deny-list. Keeping the rule here (instead of
 * inline in the route) means the writer and the gatekeeper cannot drift apart.
 */

export function screenshotKey(runId: string, stepIndex: number): string {
  return `runs/${runId}/step-${stepIndex}.png`;
}

/** Screenshot taken immediately after a restore, so a human can see the page. */
export function restoreScreenshotKey(runId: string, stepIndex: number): string {
  return `runs/${runId}/restore-${stepIndex}.png`;
}

/** Encrypted browser session state captured at a halt. Never served. */
export function sessionStateKey(runId: string, stepIndex: number): string {
  return `runs/${runId}/session/gate-${stepIndex}.bin`;
}

/** Prefix holding every session blob for a run, purged when the run ends. */
export function sessionPrefix(runId: string): string {
  return `runs/${runId}/session`;
}

const SERVABLE_FILENAME = /^(?:step|restore)-\d+\.png$/;

/**
 * Whether a key path may be streamed to a browser.
 *
 * `segments` is the already-split path. Returns the owning run id when servable
 * so the caller can enforce org ownership, or null to 404.
 */
export function servableRunId(segments: readonly string[]): string | null {
  if (segments.length !== 3) return null;
  const [prefix, runId, filename] = segments;
  if (prefix !== "runs" || !runId || !filename) return null;
  if (!SERVABLE_FILENAME.test(filename)) return null;
  // Defence against traversal or injection through any segment.
  if (segments.some((s) => s.includes("\0") || s.includes("/") || s.includes("\\") || s === ".." || s === ".")) {
    return null;
  }
  return runId;
}
