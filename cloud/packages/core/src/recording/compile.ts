import { classifyStep } from "../classifier/sensitive.js";
import type { Selector, WorkflowStep, WorkflowSteps } from "../schema/step.js";
import type { RecordingTrace, TraceEvent, TraceTarget } from "./trace.js";

/**
 * Turns a captured trace into typed workflow steps — **deterministically**.
 *
 * No model, no network, no inference. The recorder already read the accessible
 * role and name off the live element, which are the fields the worker's
 * resolution chain prefers, so compilation is a mechanical translation rather
 * than a guess. That matters for more than purity: it means recording works
 * with no compiler configured, in a deployment that has deliberately removed
 * every AI dependency from the runtime (see `docs/DEPLOY.md`).
 *
 * A model still has a job here, just not this one. Naming steps readably,
 * proposing where an approval belongs beyond the obvious cases, and flagging
 * what it could not resolve are all judgement calls. Deciding *which element
 * was clicked* is not, and should never have been.
 *
 * Three things it does beyond translation, all conservative:
 *
 * 1. **Collapses keystroke noise.** Consecutive `input` events on one field
 *    become one `fill` with the final value. A recorder that emits per
 *    keystroke would otherwise produce a step per character.
 * 2. **Gates sensitive actions.** Every produced step is run through the same
 *    `classifyStep` the engine uses, and an `approval` step is inserted before
 *    any that gates. The workflow therefore arrives already stopping before it
 *    submits, rather than relying on whoever reviews it to notice.
 * 3. **Never carries a secret.** A redacted input becomes a `fill` marked
 *    `sensitive` with a placeholder value. The real value was never in the
 *    trace; this makes its absence explicit in the step the human reviews.
 */

export interface CompileTraceResult {
  steps: WorkflowSteps;
  /** Human-facing notes: what was dropped, and what needs a decision. */
  notes: string[];
}

/** Ghost never stores a captured secret. The reviewer replaces this. */
const SECRET_PLACEHOLDER = "<redacted — set a value before running>";

function selectorFrom(target: TraceTarget): Selector | null {
  const selector: Selector = {};
  if (target.role) selector.role = target.role;
  if (target.name) selector.name = target.name;
  if (target.testId) selector.testId = target.testId;
  // Text is only worth carrying when nothing stronger identified the element;
  // as an extra field alongside role+name it adds no resolution power and
  // makes the step noisier to read.
  if (!selector.role && !selector.testId && target.text) selector.text = target.text;
  if (!selector.role && !selector.testId && !selector.text) {
    const css = target.selectorCandidates[0];
    if (css) selector.css = css;
  }
  return Object.keys(selector).length > 0 ? selector : null;
}

function describe(target: TraceTarget): string | undefined {
  if (target.name) return target.name;
  if (target.text) return target.text;
  return undefined;
}

/** Stable within one compile; the schema only requires uniqueness. */
function makeIdFactory() {
  let n = 0;
  return (prefix: string) => `${prefix}${++n}`;
}

/**
 * True when two consecutive input events are typing into the same field, so
 * only the last value matters. Compared on the resolved selector rather than
 * on object identity, since each event carries its own target object.
 */
function sameField(a: TraceEvent, b: TraceEvent): boolean {
  if (a.type !== "input" || b.type !== "input") return false;
  const sa = selectorFrom(a.target);
  const sb = selectorFrom(b.target);
  if (!sa || !sb) return false;
  return JSON.stringify(sa) === JSON.stringify(sb);
}

export function compileTrace(trace: RecordingTrace): CompileTraceResult {
  const id = makeIdFactory();
  const steps: WorkflowStep[] = [];
  const notes: string[] = [];

  // Open the page the recording started on. Without this the workflow depends
  // on whatever the browser happened to be showing.
  const firstUrl = trace.startUrl ?? trace.events.find((e) => e.url)?.url;
  if (firstUrl) {
    steps.push({ id: id("nav"), type: "navigate", url: firstUrl, label: "Open the starting page" });
  } else {
    notes.push("The trace had no URL, so the workflow does not open a page first. Add a navigate step.");
  }

  let lastUrl = firstUrl;

  for (let i = 0; i < trace.events.length; i++) {
    const event = trace.events[i]!;

    // Collapse a run of keystrokes on one field into its final value.
    if (event.type === "input") {
      let last = event;
      while (i + 1 < trace.events.length && sameField(last, trace.events[i + 1]!)) {
        last = trace.events[++i] as typeof event;
      }
      const selector = selectorFrom(last.target);
      if (!selector) {
        notes.push(
          `Skipped typing into an element with nothing to identify it by (at ${last.url}). ` +
            "The recorder could not read a role, name, test id or text for it.",
        );
        continue;
      }
      steps.push({
        id: id("fill"),
        type: "fill",
        selector,
        value: last.redacted ? SECRET_PLACEHOLDER : (last.value ?? ""),
        sensitive: last.redacted,
        ...(describe(last.target) ? { label: `Fill ${describe(last.target)}` } : {}),
      });
      if (last.redacted) {
        notes.push(
          `The value typed into "${describe(last.target) ?? "a field"}" was a password or ` +
            "other secret, so it was never captured. Set it before running this workflow.",
        );
      }
      continue;
    }

    if (event.type === "select") {
      const selector = selectorFrom(event.target);
      if (!selector) {
        notes.push(`Skipped a dropdown selection with nothing to identify it by (at ${event.url}).`);
        continue;
      }
      steps.push({
        id: id("select"),
        type: "select",
        selector,
        value: event.value,
        ...(describe(event.target) ? { label: `Choose ${describe(event.target)}` } : {}),
      });
      continue;
    }

    if (event.type === "click") {
      const selector = selectorFrom(event.target);
      if (!selector) {
        notes.push(
          `Skipped a click on an element with nothing to identify it by (at ${event.url}). ` +
            "Pixel coordinates are deliberately not recorded, so this click cannot be replayed.",
        );
        continue;
      }
      steps.push({
        id: id("click"),
        type: "click",
        selector,
        ...(describe(event.target) ? { description: describe(event.target) } : {}),
        ...(describe(event.target) ? { label: `Click ${describe(event.target)}` } : {}),
      });
      continue;
    }

    if (event.type === "navigate") {
      // Only when the page actually changed; a recorder may emit one per
      // history event, including replaces that land on the same URL.
      if (event.url && event.url !== lastUrl) {
        steps.push({
          id: id("nav"),
          type: "navigate",
          url: event.url,
          label: "Follow the page change",
        });
        lastUrl = event.url;
      }
      continue;
    }

    // `submit` carries no reliable target of its own — the click that caused
    // it is already recorded, and replaying a form submission separately would
    // double the action.
    if (event.type === "submit") {
      notes.push(
        `A form was submitted at ${event.url}. The click that submitted it is already a step; ` +
          "check that an approval sits in front of it.",
      );
      continue;
    }
  }

  const gated = insertApprovals(steps, id);

  if (gated.inserted === 0) {
    notes.push(
      "No step in this recording looked irreversible, so no approval gate was added. " +
        "If it sends, pays, deletes or submits anything, add one before publishing.",
    );
  }

  return { steps: gated.steps as WorkflowSteps, notes };
}

/**
 * Puts an `approval` immediately before every step the engine would gate.
 *
 * Uses `classifyStep` — the same deterministic classifier the worker consults
 * at run time — so the authored workflow agrees with what execution will
 * actually do. Inserting a gate here does not *create* the protection (the
 * engine gates regardless); it makes the pause visible in the plan the human
 * reviews, with a reason attached.
 */
function insertApprovals(
  steps: WorkflowStep[],
  id: (prefix: string) => string,
): { steps: WorkflowStep[]; inserted: number } {
  const out: WorkflowStep[] = [];
  let inserted = 0;

  for (const step of steps) {
    const verdict = classifyStep(step);
    if (verdict.sensitive) {
      const previous = out[out.length - 1];
      // Do not stack two gates: an approval immediately before is already the
      // pause this would add.
      if (!previous || previous.type !== "approval") {
        out.push({
          id: id("approve"),
          type: "approval",
          reason: verdict.reason ?? "This step changes something outside Ghost.",
        });
        inserted++;
      }
    }
    out.push(step);
  }

  return { steps: out, inserted };
}
