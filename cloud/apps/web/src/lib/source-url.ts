/**
 * Where this instance's Corresponding Source lives.
 *
 * Ghost is AGPL-3.0-or-later, and section 13 obliges anyone running a *modified*
 * Ghost as a network service to offer that service's users the source of the
 * version they are actually talking to. A hardcoded upstream link would satisfy
 * that only for unmodified deployments and would quietly make every fork
 * non-compliant, so the URL is configurable and the upstream repo is merely the
 * default.
 *
 * Self-hosters running a fork should set `NEXT_PUBLIC_SOURCE_URL` to their own
 * published source. Pointing it at a specific tag or commit is better than a
 * bare repo root, since section 13 is about the running version.
 *
 * Read at module scope rather than per-call: `NEXT_PUBLIC_*` is inlined at build
 * time, so there is nothing to re-read at runtime.
 */
export const SOURCE_URL =
  process.env.NEXT_PUBLIC_SOURCE_URL?.trim() || "https://github.com/mohabbis/ghost";
