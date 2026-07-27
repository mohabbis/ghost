import assert from "node:assert/strict";
import test from "node:test";

import { renderActionPlanSteps } from "./review.js";

test("renders and escapes the planner reason in the approval preview", () => {
  const html = renderActionPlanSteps({
    version: 1,
    id: "plan-1",
    title: "Organizer preview",
    source: {},
    summary: {
      total_steps: 1,
      filesystem_steps: 1,
      ui_steps: 0,
      verify_steps: 0,
      denied: 0,
      needs_confirmation: 1,
    },
    steps: [
      {
        id: "step-1",
        label: "Move report.pdf",
        kind: { kind: "move_file", from: "/in/report.pdf", to: "/out/report.pdf" },
        capability: {},
        decision: {
          decision: "require_confirmation",
          risk: "medium",
        },
        reason: "`.pdf` -> Documents <review>",
      },
    ],
  });

  assert.match(html, /ghost2-step__reason/);
  assert.match(html, /`\.pdf` -&gt; Documents &lt;review&gt;/);
  assert.doesNotMatch(html, /<review>/);
});
