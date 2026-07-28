import type { WorkflowSteps } from "@ghost/core/schema/step";
import { appUrl } from "./app-url";

/**
 * The seed demo workflow. Runs entirely against Ghost's own bundled fixture page
 * (`/fixtures/order`) so it is demoable with no external site or credentials.
 * The "Submit order" click is flagged sensitive by the classifier, so the run
 * halts for approval before submitting — exercising the whole trust loop.
 */
export function demoWorkflowSteps(): WorkflowSteps {
  const base = appUrl();
  return [
    { id: "nav", type: "navigate", url: `${base}/fixtures/order`, label: "Open order form" },
    {
      id: "fill",
      type: "fill",
      selector: { role: "textbox", name: "Full name" },
      value: "Ada Lovelace",
      sensitive: false,
      label: "Enter customer name",
    },
    {
      id: "submit",
      type: "click",
      selector: { role: "button", name: "Submit order" },
      label: "Submit order",
    },
    {
      id: "verify",
      type: "verify",
      assertion: { kind: "textPresent", expected: "Order submitted" },
      label: "Confirm submission",
    },
  ];
}

export const DEMO_WORKFLOW_NAME = "Demo: submit an order";
export const DEMO_WORKFLOW_DESCRIPTION =
  "Fills and submits Ghost's bundled order-form fixture. The submit click requires approval.";
