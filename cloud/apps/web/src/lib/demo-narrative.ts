/**
 * The demo workflow, narrated for the signed-out marketing surfaces.
 *
 * Deliberately derived from the *same* four steps `demo-workflow.ts` actually
 * executes (`navigate` → `fill` → `click` → `verify`), because the landing page
 * and `/demo` are the two places most likely to drift into describing a product
 * that does not exist. Rule 10 in CLAUDE.md: marketing must not promise
 * capabilities the app cannot support. Keeping the narrative anchored to the
 * real seed workflow makes that drift a code change rather than a copy edit.
 *
 * These are illustrations of a run, not a recording of one. Every surface that
 * renders them says so — see `DEMO_DISCLAIMER`. Nothing here is fetched, and
 * nothing here should ever be presented as live tenant data.
 */

export type DemoStepState = "succeeded" | "approval" | "pending";

export interface DemoStep {
  /** Matches the `id` of the corresponding step in `demoWorkflowSteps()`. */
  id: string;
  /** The step's `type` in the workflow schema. */
  type: string;
  label: string;
  /** What the step did, in the words a reviewer would use. */
  detail: string;
  state: DemoStepState;
  /** Wall-clock duration, for texture in the timeline. */
  duration?: string;
}

export const DEMO_STEPS: DemoStep[] = [
  {
    id: "nav",
    type: "navigate",
    label: "Open order form",
    detail: "Opened the order page in a real Chrome browser.",
    state: "succeeded",
    duration: "0.8s",
  },
  {
    id: "fill",
    type: "fill",
    label: "Enter customer name",
    detail: 'Typed "Ada Lovelace" into the Full name box — found by its label, not its position on screen.',
    state: "succeeded",
    duration: "0.2s",
  },
  {
    id: "submit",
    type: "click",
    label: "Submit order",
    detail:
      "Clicking this actually places the order, so Ghost stopped. It will not click until a person approves.",
    state: "approval",
  },
  {
    id: "verify",
    type: "verify",
    label: "Confirm submission",
    detail: 'Will check the page really says "Order submitted". If it doesn\'t, the run is marked failed.',
    state: "pending",
  },
];

/**
 * What Ghost does to a workflow, in order.
 *
 * Written as plain mechanical description on purpose. Earlier copy for these
 * surfaces leaned on abstractions ("governed execution", "the trust pipeline")
 * that describe a posture rather than a behaviour, and a reader could finish
 * the page without learning what the software does to a form. Each stage below
 * names an action and its observable consequence.
 *
 * `built: false` marks a stage that is designed and schema-backed but not yet
 * a finished product surface, so the page can say so rather than implying the
 * whole loop ships today.
 */
export interface PipelineStage {
  /** Plain verb for what happens. */
  name: string;
  /** One line, concrete, no abstractions. */
  blurb: string;
  built: boolean;
}

export const PIPELINE: PipelineStage[] = [
  {
    name: "You do the task once",
    blurb:
      "Click through it in your browser while Ghost watches — every click, keystroke, and page it lands on.",
    built: false,
  },
  {
    name: "It becomes a list of steps",
    blurb:
      'Not a video. A readable list: "open this page", "type this in the Full name box", "click Submit". You can edit any line.',
    built: true,
  },
  {
    name: "Ghost runs the steps",
    blurb:
      "A real Chrome browser on a server does the clicking and typing, finding buttons by their label rather than screen position.",
    built: true,
  },
  {
    name: "It stops before anything irreversible",
    blurb:
      "Submit, send, pay, delete — Ghost pauses and waits for you to click Approve. It cannot proceed on its own.",
    built: true,
  },
  {
    name: "It checks the result",
    blurb:
      'After a step, Ghost confirms what it expected actually happened — "the page now says Order submitted" — or the run fails.',
    built: true,
  },
  {
    name: "It writes down what it did",
    blurb:
      "Every step, screenshot, and approval goes into a log that is chained so an edited or deleted entry shows up as tampering.",
    built: true,
  },
  {
    name: "It can walk changes back",
    blurb:
      "Undoable actions get undone on request. When Ghost can't tell whether something landed, it stops and asks instead of guessing.",
    built: true,
  },
];

/** Shown wherever `DEMO_STEPS` is rendered. Not optional. */
export const DEMO_DISCLAIMER =
  "An illustration of the bundled demo workflow, not a recording of a live run.";
