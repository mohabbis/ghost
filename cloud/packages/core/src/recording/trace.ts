import { z } from "zod";

/**
 * The capture format: what a Ghost recorder uploads.
 *
 * This is a **contract**, not an internal shape. The Chrome extension in
 * `apps/extension` produces it; `compileTrace` consumes it. Anything else that
 * can produce this JSON — a future desktop recorder, a customer's own tooling —
 * works without changing Ghost.
 *
 * The design decision that matters: a recorder emits **accessible role and
 * name at capture time**, computed from the live DOM while the element is
 * still on screen. Those are exactly the fields `resolveLocator` prefers, so
 * the trace can be compiled into steps deterministically, with no model in the
 * correctness path. Inferring `role`/`name` later from an opaque HAR or event
 * log is guesswork; reading them off the element is not.
 *
 * `selectorCandidates` is ordered best-first and exists because the worker
 * replays in a *different* browser from the one that recorded. A single
 * brittle selector is the failure that design invites.
 */

/** What a recorder observed about the element an event happened on. */
export const traceTargetSchema = z.object({
  /** ARIA role, explicit or implicit. */
  role: z.string().min(1).optional(),
  /** Accessible name, computed the way a screen reader would. */
  name: z.string().min(1).optional(),
  /** `data-testid` and friends, when the page has them. */
  testId: z.string().min(1).optional(),
  /** Visible text, as a weaker fallback than role+name. */
  text: z.string().min(1).optional(),
  /**
   * Ordered best-first CSS fallbacks. Only used when nothing semantic
   * resolved — the resolution chain prefers role+name, then testId, then text.
   */
  selectorCandidates: z.array(z.string().min(1)).max(10).default([]),
  /** Tag name, for deciding between `fill` and `select`. */
  tagName: z.string().min(1).optional(),
  /** `type` attribute of an input, for sensitivity decisions. */
  inputType: z.string().min(1).optional(),
});

export type TraceTarget = z.infer<typeof traceTargetSchema>;

const base = z.object({
  /** Page URL at the moment of the event. */
  url: z.string().min(1),
  /** Epoch milliseconds. Ordering is by array position; this is for humans. */
  timestamp: z.number().int().nonnegative(),
});

/**
 * `redacted` is set by the *recorder*, which is the only place it can be done
 * honestly: the value never leaves the page. A trace that carried the secret
 * plus a "please ignore this" flag would already have leaked it — see
 * `docs/DEPLOY.md` on why a trace is customer data.
 */
export const traceEventSchema = z.discriminatedUnion("type", [
  base.extend({
    type: z.literal("navigate"),
  }),
  base.extend({
    type: z.literal("click"),
    target: traceTargetSchema,
  }),
  base.extend({
    type: z.literal("input"),
    target: traceTargetSchema,
    /** Absent when `redacted` — the recorder never captured it. */
    value: z.string().optional(),
    redacted: z.boolean().default(false),
  }),
  base.extend({
    type: z.literal("select"),
    target: traceTargetSchema,
    value: z.string(),
  }),
  base.extend({
    type: z.literal("submit"),
    target: traceTargetSchema.optional(),
  }),
]);

export type TraceEvent = z.infer<typeof traceEventSchema>;

export const recordingTraceSchema = z.object({
  version: z.literal(1),
  sessionId: z.string().min(1),
  /** Where recording started, so the compiled workflow opens the right page. */
  startUrl: z.string().min(1).optional(),
  capturedAt: z.number().int().nonnegative().optional(),
  /** Names the producer, so a bad recorder can be identified from a trace. */
  recorder: z.string().min(1).optional(),
  events: z.array(traceEventSchema),
});

export type RecordingTrace = z.infer<typeof recordingTraceSchema>;

/**
 * Whether some uploaded bytes are a structured Ghost trace.
 *
 * Used to decide between the deterministic compiler and the optional
 * model-backed one. A HAR or a Playwright zip is not this shape and falls
 * through to whatever compiler is configured, if any.
 */
export function parseRecordingTrace(
  input: unknown,
): { ok: true; trace: RecordingTrace } | { ok: false; error: string } {
  const parsed = recordingTraceSchema.safeParse(input);
  if (!parsed.success) {
    const first = parsed.error.issues[0];
    return {
      ok: false,
      error: first ? `${first.path.join(".") || "trace"}: ${first.message}` : "invalid trace",
    };
  }
  return { ok: true, trace: parsed.data };
}
