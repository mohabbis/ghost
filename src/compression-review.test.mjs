// Behavioral tests for the pure (DOM-free) logic in CompressionReview.
//
// Run with: node --test src/compression-review.test.mjs
//
// No test framework or package.json is introduced — this uses Node's
// built-in test runner (`node:test`), and imports compression-review.js
// directly. Node 22 auto-detects the file's existing `export class` syntax
// as ESM with no config; see CLAUDE.md for why this repo intentionally has
// no bundler or package.json.
//
// `document`/`window` are stubbed minimally at module scope only so the
// constructor (which reads `document.getElementById`) and `lastRunOutcomes`
// (which reads `window.__ghostLastReplayTrace`) don't throw in Node. Every
// method under test here is otherwise pure: given inputs, it returns a
// value with no other side effects.

import { test } from "node:test";
import assert from "node:assert/strict";

globalThis.document = {
  getElementById: () => null,
  createElement: (tag) => {
    const el = { tagName: tag.toUpperCase(), textContent: "", innerHTML: "" };
    Object.defineProperty(el, "textContent", {
      set(v) {
        this.innerHTML = String(v ?? "")
          .replace(/&/g, "&amp;")
          .replace(/</g, "&lt;")
          .replace(/>/g, "&gt;")
          .replace(/"/g, "&quot;");
      },
      get() {
        return this.innerHTML
          .replace(/&lt;/g, "<")
          .replace(/&gt;/g, ">")
          .replace(/&quot;/g, '"')
          .replace(/&amp;/g, "&");
      },
    });
    return el;
  },
};
globalThis.window = {};

const { CompressionReview } = await import("./compression-review.js");

function review() {
  return new CompressionReview(undefined);
}

test("getStepIcon returns the label for each known kind and a fallback otherwise", () => {
  const r = review();
  assert.equal(r.getStepIcon("click"), "Click");
  assert.equal(r.getStepIcon("type_text"), "Type");
  assert.equal(r.getStepIcon("shortcut"), "Key");
  assert.equal(r.getStepIcon("scroll"), "Scroll");
  assert.equal(r.getStepIcon("wait"), "Wait");
  assert.equal(r.getStepIcon("unknown"), "?");
  assert.equal(r.getStepIcon("something_new"), "•");
});

test("getStepDescription: click with a resolved target names it", () => {
  const r = review();
  const desc = r.getStepDescription({
    kind: "click",
    target: { name: "Send", role: "AXButton" },
  });
  assert.equal(desc, 'Clicked <strong>"Send"</strong> (AXButton)');
});

test("getStepDescription: click with only fallback coordinates reports the point", () => {
  const r = review();
  const desc = r.getStepDescription({ kind: "click", fallback_coords: [120, 340] });
  assert.equal(desc, "Clicked at (120, 340)");
});

test("getStepDescription: click with neither target nor coordinates degrades to a bare label", () => {
  const r = review();
  assert.equal(r.getStepDescription({ kind: "click" }), "Clicked");
});

test("getStepDescription: secure-field typed text is always redacted, never shown", () => {
  const r = review();
  const desc = r.getStepDescription({
    kind: "type_text",
    secure_field: true,
    char_count: 8,
    text: "hunter22",
  });
  assert.equal(desc, "Typed <em>[redacted: 8 chars in secure field]</em>");
  assert.ok(!desc.includes("hunter22"), "raw secure-field text must never appear");
});

test("getStepDescription: redacted (non-secure) typed text hides the text", () => {
  const r = review();
  const desc = r.getStepDescription({ kind: "type_text", redacted: true, char_count: 12 });
  assert.equal(desc, "Typed <em>[redacted: 12 chars]</em>");
});

test("getStepDescription: retained typed text is shown, truncated", () => {
  const r = review();
  const desc = r.getStepDescription({
    kind: "type_text",
    text: "a".repeat(50),
    char_count: 50,
  });
  assert.equal(desc, `Typed <strong>"${"a".repeat(40)}…"</strong>`);
});

test("getStepDescription: typed text with no retained text falls back to a char count", () => {
  const r = review();
  assert.equal(
    r.getStepDescription({ kind: "type_text", char_count: 5 }),
    "Typed (5 chars)"
  );
});

test("getStepDescription: shortcut, scroll, wait, and unknown kinds", () => {
  const r = review();
  assert.equal(r.getStepDescription({ kind: "shortcut", combo: "⌘C" }), "Pressed <strong>⌘C</strong>");
  assert.equal(
    r.getStepDescription({ kind: "scroll", direction: "down", magnitude: "large" }),
    "Scrolled down large"
  );
  assert.equal(r.getStepDescription({ kind: "wait", ms: 500 }), "Waited 500ms");
  assert.equal(
    r.getStepDescription({ kind: "unknown", description: "raw event" }),
    "Unknown: raw event"
  );
});

test("getStepDescription: an unrecognized kind never silently drops the step", () => {
  const r = review();
  const step = { kind: "future_kind", value: 1 };
  assert.equal(
    r.getStepDescription(step),
    '{&quot;kind&quot;:&quot;future_kind&quot;,&quot;value&quot;:1}',
  );
});

test("getWarningText covers every warning kind and falls back for unknown ones", () => {
  const r = review();
  assert.match(
    r.getWarningText({ kind: "coordinate_only_target", step_index: 2 }),
    /Step 3: coordinate-only target/
  );
  assert.match(
    r.getWarningText({ kind: "low_confidence", step_index: 0, confidence: 0.42 }),
    /Step 1: low confidence \(42%\)/
  );
  assert.match(
    r.getWarningText({ kind: "secure_field_typing", step_index: 4 }),
    /Step 5: typing in secure field/
  );
  const other = { kind: "mystery", step_index: 1 };
  assert.equal(
    r.getWarningText(other),
    '{&quot;kind&quot;:&quot;mystery&quot;,&quot;step_index&quot;:1}',
  );
});

test("getRiskClass: unknown kind takes precedence over confidence", () => {
  const r = review();
  assert.equal(r.getRiskClass({ kind: "unknown", confidence: 0.9 }), "step-unknown");
});

test("getRiskClass: low confidence is flagged regardless of step kind", () => {
  const r = review();
  assert.equal(r.getRiskClass({ kind: "click", confidence: 0.3 }), "step-low-confidence");
});

test("getRiskClass: secure-field typing is flagged even at high confidence", () => {
  const r = review();
  assert.equal(
    r.getRiskClass({ kind: "type_text", secure_field: true, confidence: 0.99 }),
    "step-secure"
  );
});

test("getRiskClass: an ordinary confident step is normal", () => {
  const r = review();
  assert.equal(r.getRiskClass({ kind: "click", confidence: 0.95 }), "step-normal");
});

test("truncate leaves short strings alone and ellipsizes long ones", () => {
  const r = review();
  assert.equal(r.truncate("short", 40), "short");
  assert.equal(r.truncate("a".repeat(40), 40), "a".repeat(40));
  assert.equal(r.truncate("a".repeat(41), 40), "a".repeat(40) + "…");
});

test("lastRunOutcomes: no replay handoff yields an empty map", () => {
  const r = review();
  r.eventCount = 3;
  r.report = { raw_spans: [[0, 3]] };
  globalThis.window.__ghostLastReplayTrace = undefined;
  assert.equal(r.lastRunOutcomes().size, 0);
});

test("lastRunOutcomes: a stale handoff for a different event list is ignored", () => {
  const r = review();
  r.eventCount = 3;
  r.report = { raw_spans: [[0, 3]] };
  globalThis.window.__ghostLastReplayTrace = {
    eventCount: 99,
    trace: [{ kind: "CoordinateFallback", step_index: 0 }],
  };
  assert.equal(r.lastRunOutcomes().size, 0);
});

test("lastRunOutcomes: maps a fallback raw index onto its compressed step via non-contiguous spans", () => {
  const r = review();
  // Step 0 covers raw indices 0-1, step 1 covers raw indices 5-7; indices
  // 2-4 belong to no step (dropped delays / bare releases), exactly the
  // "spans are non-contiguous" case the source comment warns about.
  r.eventCount = 8;
  r.report = { raw_spans: [[0, 2], [5, 3]] };
  globalThis.window.__ghostLastReplayTrace = {
    eventCount: 8,
    trace: [
      { kind: "CoordinateFallback", step_index: 1 },
      { kind: "NoDescriptor", step_index: 6 },
    ],
  };
  const outcomes = r.lastRunOutcomes();
  assert.equal(outcomes.size, 2);
  assert.equal(outcomes.get(0), "lost its element last run — clicked recorded coordinates");
  assert.equal(outcomes.get(1), "ran on raw coordinates last run");
});

test("lastRunOutcomes: a raw index outside every span is silently skipped, not mis-attributed", () => {
  const r = review();
  r.eventCount = 8;
  r.report = { raw_spans: [[0, 2], [5, 3]] };
  globalThis.window.__ghostLastReplayTrace = {
    eventCount: 8,
    trace: [{ kind: "CoordinateFallback", step_index: 3 }],
  };
  assert.equal(r.lastRunOutcomes().size, 0);
});

test("lastRunOutcomes: only the first fallback per step is kept", () => {
  const r = review();
  r.eventCount = 2;
  r.report = { raw_spans: [[0, 2]] };
  globalThis.window.__ghostLastReplayTrace = {
    eventCount: 2,
    trace: [
      { kind: "CoordinateFallback", step_index: 0 },
      { kind: "NoDescriptor", step_index: 1 },
    ],
  };
  const outcomes = r.lastRunOutcomes();
  assert.equal(outcomes.size, 1);
  assert.equal(outcomes.get(0), "lost its element last run — clicked recorded coordinates");
});

test("guardFindingsForStep maps findings onto compressed step indices", () => {
  const r = review();
  r.guardReport = {
    findings: [
      { step_index: 1, severity: "high", title: "Sensitive field" },
      { step_index: 1, severity: "medium", title: "Coordinate fallback" },
      { step_index: 3, severity: "low", title: "Long wait" },
    ],
  };
  const notes = r.guardFindingsForStep(1);
  assert.equal(notes.length, 2);
  assert.match(notes[0], /Sensitive field/);
  assert.match(notes[1], /Coordinate fallback/);
  assert.equal(r.guardFindingsForStep(0).length, 0);
  assert.equal(r.guardFindingsForStep(3).length, 1);
});

test("guardSeverityLabel uses escalating markers by severity", () => {
  const r = review();
  assert.equal(r.guardSeverityLabel("low"), "·");
  assert.equal(r.guardSeverityLabel("critical"), "!!!");
});

test("combinedReviewNote merges policy and guard reasons", () => {
  const r = review();
  const note = r.combinedReviewNote(
    1,
    "OS click outside approved zone",
    ["!! Sensitive field", "· Coordinate fallback"],
  );
  assert.match(note, /Policy: OS click outside approved zone/);
  assert.match(note, /Guard: Sensitive field/);
  assert.match(note, /Coordinate fallback/);
});

test("routineDecisionBadge maps policy decisions to Organizer-style badges", () => {
  const r = review();
  assert.match(r.routineDecisionBadge({ decision: "allow" }), /Allowed/);
  assert.match(
    r.routineDecisionBadge({ decision: "deny", reason: "outside zone" }),
    /Denied/
  );
  assert.match(
    r.routineDecisionBadge({ decision: "require_confirmation", risk: "high" }),
    /Needs approval/
  );
  assert.equal(r.routineDecisionBadge(null), "");
});

test("policySummaryText prefers denied then confirm then allow counts", () => {
  const r = review();
  r.policyPlan = { denied_count: 2, confirmation_count: 1, allow_count: 3 };
  assert.equal(r.policySummaryText(), "2 denied");
  r.policyPlan = { denied_count: 0, confirmation_count: 4, allow_count: 3 };
  assert.equal(r.policySummaryText(), "4 confirm");
  r.policyPlan = { denied_count: 0, confirmation_count: 0, allow_count: 5 };
  assert.equal(r.policySummaryText(), "5 allowed");
});

test("renderSensitiveSuppressionNote surfaces password/OTP suppression", () => {
  const r = review();
  r.report = { redacted_fields: 0 };
  r.guardReport = {
    findings: [{ category: "sensitive_field", severity: "high", title: "Password field" }],
  };
  const note = r.renderSensitiveSuppressionNote();
  assert.match(note, /Password, OTP, and payment keystrokes were suppressed/);
  assert.match(note, /data-guard-suppression-note/);

  r.guardReport = { findings: [] };
  r.report = { redacted_fields: 2 };
  assert.match(r.renderSensitiveSuppressionNote(), /suppressed/);

  r.report = { redacted_fields: 0 };
  assert.equal(r.renderSensitiveSuppressionNote(), "");
});

test("getRiskClass: policy deny takes precedence over compression heuristics", () => {
  const r = review();
  assert.equal(
    r.getRiskClass({ kind: "click", confidence: 0.99 }, { decision: "deny" }),
    "step-policy-deny"
  );
  assert.equal(
    r.getRiskClass(
      { kind: "click", confidence: 0.99 },
      { decision: "require_confirmation", risk: "high" }
    ),
    "step-policy-confirm step-policy-confirm-high"
  );
  assert.equal(
    r.getRiskClass({ kind: "unknown", confidence: 0.1 }, { decision: "allow" }),
    "step-unknown"
  );
});
