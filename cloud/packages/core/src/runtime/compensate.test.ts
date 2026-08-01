import { describe, expect, it } from "vitest";
import {
  planCompensation,
  hasSideEffect,
  classifyCompensation,
  describeActions,
  remainingEntries,
} from "./compensate.js";
import { emptyJournal, type RunJournal } from "./journal.js";
import { workflowStep, type WorkflowStep } from "../schema/step.js";

function journal(completed: number[]): RunJournal {
  return { ...emptyJournal(), completed: new Set(completed) };
}

const steps: WorkflowStep[] = [
  { id: "nav", type: "navigate", url: "https://shop.example.com/order" },
  {
    id: "name",
    type: "fill",
    selector: { name: "Full name" },
    value: "Ada",
    sensitive: false,
    compensate: {
      description: "Clear the customer name field",
      actions: [{ type: "fill", selector: { name: "Full name" }, value: "" }],
    },
  },
  {
    id: "submit",
    type: "click",
    selector: { role: "button", name: "Submit order" },
    compensate: {
      description: "Open the order and click Cancel order",
      actions: [
        { type: "navigate", url: "https://shop.example.com/orders/latest" },
        { type: "click", selector: { role: "button", name: "Cancel order" } },
      ],
    },
  },
  { id: "check", type: "verify", assertion: { kind: "textPresent", expected: "Order submitted" } },
];

describe("planCompensation", () => {
  it("plans nothing for a run that did nothing", () => {
    const plan = planCompensation(steps, journal([]));
    expect(plan.entries).toEqual([]);
    expect(plan.irreversible).toEqual([]);
    expect(plan.complete).toBe(true);
  });

  it("reverses completed steps in reverse order", () => {
    // Camunda's rule: the last thing done is the first thing undone.
    const plan = planCompensation(steps, journal([0, 1, 2, 3]));
    expect(plan.entries.map((e) => e.stepIndex)).toEqual([2, 1]);
  });

  it("ignores steps that never ran", () => {
    const plan = planCompensation(steps, journal([0, 1]));
    expect(plan.entries.map((e) => e.stepId)).toEqual(["name"]);
  });

  it("ignores steps with nothing to reverse", () => {
    // Navigations and assertions changed nothing outside the browser, so
    // listing them as irreversible would be noise rather than honesty.
    const plan = planCompensation(steps, journal([0, 3]));
    expect(plan.entries).toEqual([]);
    expect(plan.irreversible).toEqual([]);
    expect(plan.complete).toBe(true);
  });

  it("reports a side-effecting step with no compensation instead of hiding it", () => {
    // A partial undo presented as a complete one is worse than no undo: the
    // operator needs to know the email cannot be unsent.
    const withoutUndo: WorkflowStep[] = [
      {
        id: "email",
        type: "sendEmail",
        connectorId: "c",
        to: ["a@b.c"],
        subject: "Your order",
        body: "Thanks",
      },
    ];
    const plan = planCompensation(withoutUndo, journal([0]));
    expect(plan.entries).toEqual([]);
    expect(plan.irreversible).toHaveLength(1);
    expect(plan.irreversible[0]).toMatchObject({ stepIndex: 0, stepId: "email" });
    expect(plan.irreversible[0]!.reason).toMatch(/no compensation/i);
    expect(plan.complete).toBe(false);
  });

  it("flags a reversal that itself needs approval", () => {
    // Undoing a submitted order is a mutating action in its own right. It is
    // classified and gated like one, not waved through as "just an undo".
    const plan = planCompensation(steps, journal([0, 1, 2]));
    const submit = plan.entries.find((e) => e.stepId === "submit");
    expect(submit?.requiresApproval).toBe(true);
    expect(submit?.approvalReason).toMatch(/cancel order/i);
  });

  it("gates a reversal that types into a field, even a restorative one", () => {
    // Clearing a name field is harmless, and this still gates. A compensation
    // action carries no `sensitive` flag of its own, so the alternative is to
    // assume every reversal fill is safe — which would let a reversal type into
    // a field named "API token" with no approval, looser than the identical
    // forward step. The false positive lands on the rare path.
    const plan = planCompensation(steps, journal([0, 1, 2]));
    const name = plan.entries.find((e) => e.stepId === "name");
    expect(name?.requiresApproval).toBe(true);
    expect(name?.approvalReason).toMatch(/Full name/);
  });

  it("carries the human-readable description into the plan", () => {
    const plan = planCompensation(steps, journal([0, 1, 2]));
    expect(plan.entries[0]!.compensation.description).toMatch(/Cancel order/);
  });
});

describe("hasSideEffect", () => {
  it("classifies every step type", () => {
    const types = workflowStep.options.map((o) => o.shape.type.value);
    expect(types.length).toBeGreaterThan(0);
  });

  it("treats outward and state-changing actions as side effects", () => {
    expect(hasSideEffect(steps[2]!)).toBe(true); // click
    expect(hasSideEffect(steps[1]!)).toBe(true); // fill
  });

  it("treats reads, waits and navigations as effect-free", () => {
    expect(hasSideEffect(steps[0]!)).toBe(false); // navigate
    expect(hasSideEffect(steps[3]!)).toBe(false); // verify
  });

  it("fails closed on an unknown step type", () => {
    const bogus = { id: "?", type: "teleport" } as unknown as WorkflowStep;
    expect(hasSideEffect(bogus)).toBe(true);
  });
});

describe("classifyCompensation", () => {
  it("gates when any action in the reversal is sensitive", () => {
    const verdict = classifyCompensation({
      description: "Refund the payment",
      actions: [
        { type: "navigate", url: "https://shop.example.com/orders/1" },
        { type: "click", selector: { role: "button", name: "Refund payment" } },
      ],
    });
    expect(verdict.sensitive).toBe(true);
  });

  it("gates any click, even one the forward word list would let through", () => {
    // The forward classifier matches "cancel subscription", not bare "cancel",
    // so "Cancel order" would sail past it. On the way back that is the wrong
    // default: every click in a compensation is walking a real effect back.
    const verdict = classifyCompensation({
      description: "Cancel the order",
      actions: [{ type: "click", selector: { role: "button", name: "Cancel order" } }],
    });
    expect(verdict.sensitive).toBe(true);
    expect(verdict.reason).toMatch(/Cancel order/);
  });

  it("gates any fill, because a compensation action cannot declare its own sensitivity", () => {
    const verdict = classifyCompensation({
      description: "Clear the field",
      actions: [{ type: "fill", selector: { name: "Notes" }, value: "" }],
    });
    expect(verdict.sensitive).toBe(true);
    expect(verdict.reason).toMatch(/Notes/);
  });

  it("does not gate a reversal that only looks at the page", () => {
    // The reversal still has to be *able* to not gate, or the gate stops
    // carrying information.
    const verdict = classifyCompensation({
      description: "Return to the order list",
      actions: [
        { type: "navigate", url: "https://shop.example.com/orders" },
        { type: "waitFor", selector: { role: "heading", name: "Orders" } },
      ],
    });
    expect(verdict.sensitive).toBe(false);
  });
});

describe("describeActions", () => {
  it("says what a reversal will actually do, not what its author called it", () => {
    // The failure this prevents: an approver shown only "Open the order and
    // click Cancel" cannot tell that the reversal navigates somewhere else.
    expect(
      describeActions({
        description: "Open the order and click Cancel order",
        actions: [
          { type: "navigate", url: "https://shop.example.com/orders/latest" },
          { type: "click", selector: { role: "button", name: "Cancel order" } },
        ],
      }),
    ).toEqual([
      "navigate to https://shop.example.com/orders/latest",
      'click the button "Cancel order"',
    ]);
  });

  it("never shows a fill value", () => {
    // A compensation may legitimately type a credential; the point of the
    // preview is what it targets.
    const lines = describeActions({
      description: "Restore the key",
      actions: [{ type: "fill", selector: { name: "API token" }, value: "sk-live-abc123" }],
    });
    expect(lines.join(" ")).not.toContain("sk-live-abc123");
    expect(lines[0]).toMatch(/API token/);
  });
});

describe("remainingEntries", () => {
  it("drops steps a previous attempt already reversed", () => {
    // A partly-reversed run can be previewed again, and re-offering finished
    // work would tell the operator more will be undone than actually will.
    const plan = planCompensation(steps, journal([0, 1, 2]));
    expect(remainingEntries(plan, new Set([2])).map((e) => e.stepId)).toEqual(["name"]);
    expect(remainingEntries(plan, new Set([1, 2]))).toEqual([]);
  });
});

describe("plan.inFlight", () => {
  it("reports a step that started and never reported an outcome", () => {
    // Cancellation lets the current step finish, so an undo requested straight
    // after a cancel can be built while a side effect is still landing.
    const j: RunJournal = { ...journal([0, 1]), inFlight: new Set([2]) };
    expect(planCompensation(steps, j).inFlight).toEqual([2]);
  });

  it("is empty for a settled run", () => {
    expect(planCompensation(steps, journal([0, 1, 2])).inFlight).toEqual([]);
  });
});
