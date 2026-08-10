import { chromium, type Browser, type BrowserContext, type Page } from "playwright";
import type { WorkflowStep } from "@ghost/core/schema/step";
import { assertPublicHttpUrl } from "@ghost/core/net/public-url";
import { discoverChromium } from "./chromium.js";
import { resolveLocator } from "./selector.js";
import { runVerification, type VerifyResult } from "./verify.js";
import type { RestorePlan } from "../runtime/restore.js";

export interface StepResult {
  /**
   * PNG bytes, or `null` when the step must not be captured (sensitive fills).
   * The caller skips the artifact store when this is null so OTP/card pixels
   * never land in the blob store.
   */
  screenshot: Buffer | null;
  verification: VerifyResult | null;
  /** Values this step captured, by name. Only `extract` produces any. */
  outputs: Record<string, string>;
  /** `page.url()` after the action, recorded so a resume can restore to it. */
  url: string;
}

export interface StepOptions {
  timeoutMs: number;
}

export function launchOptions() {
  // In this repo's environments Chromium is preinstalled; resolve an explicit
  // executable path so we never trigger a download or hit a revision mismatch.
  //
  // The fallback belongs *here*, at the only place that launches a browser,
  // rather than in each entrypoint. It used to be opt-in via a
  // `useDiscoveredChromium()` call, which the four test files made and the
  // worker's own entrypoint did not — so in exactly the environments the
  // resolver exists for (CI runners, dev containers) the suite passed while a
  // real run died at its first step on "Executable doesn't exist at
  // .../chromium_headless_shell-<rev>/...". An explicit env var still wins.
  const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE || discoverChromium();
  return { headless: true, executablePath, args: ["--no-sandbox"] };
}

/**
 * A single browser session for one run. Phase 1 uses an ephemeral context per
 * run; a persistent per-org context (for stored cookies/credentials) arrives
 * with recording in Phase 2. Even then, a session cannot be held open across a
 * three-day approval wait, so capture/restore stays the floor.
 */
export class BrowserSession {
  private constructor(
    private readonly browser: Browser,
    readonly context: BrowserContext,
    readonly page: Page,
  ) {}

  static async launch(storageState?: Buffer): Promise<BrowserSession> {
    const browser = await chromium.launch(launchOptions());
    const context = await browser.newContext(
      storageState
        ? { storageState: JSON.parse(storageState.toString("utf8")) as never }
        : undefined,
    );
    const page = await context.newPage();
    const defaultTimeout = Number(process.env.GHOST_STEP_TIMEOUT_MS) || 30_000;
    page.setDefaultTimeout(defaultTimeout);
    return new BrowserSession(browser, context, page);
  }

  async close(): Promise<void> {
    await this.browser.close();
  }
}

/**
 * Step types `applyStep` has no executor for — they fall through to `return {}`
 * and perform nothing.
 *
 * Distinct from the other types that also return `{}`: `verify` and `approval`
 * are handled elsewhere (verification in `runStep`, the gate in the state
 * machine), so their no-op here is correct. These two are simply not built.
 *
 * `EDITABLE_STEP_TYPES` in @ghost/core must never include one of these, or the
 * editor would let an author add a step that quietly does nothing while the run
 * reports success. `editable-steps.test.ts` enforces that. Remove a type from
 * this list when its executor lands, not before.
 */
export const UNIMPLEMENTED_ACTION_TYPES = ["apiCall", "sendEmail"] as const;

/**
 * Apply one workflow step's browser action (no screenshot / verification).
 * `approval` and connector steps (`apiCall`/`sendEmail`) do not act on the page
 * in Phase 1 — the approval gate is handled by the state machine, and connector
 * execution lands in a later phase.
 */
export async function applyStep(
  page: Page,
  step: WorkflowStep,
  opts: StepOptions,
): Promise<Record<string, string>> {
  const timeout = opts.timeoutMs;

  switch (step.type) {
    case "navigate":
      // Defense in depth: schema already rejects private hosts at author time,
      // but a version written before that check — or a hand-edited row — must
      // still not reach metadata endpoints from the worker.
      assertPublicHttpUrl(step.url);
      await page.goto(step.url, { waitUntil: "domcontentloaded", timeout });
      return {};
    case "click":
      await resolveLocator(page, step.selector).first().click({ timeout });
      return {};
    case "fill":
      await resolveLocator(page, step.selector).first().fill(step.value, { timeout });
      return {};
    case "select":
      await resolveLocator(page, step.selector).first().selectOption(step.value, { timeout });
      return {};
    case "waitFor":
      if (step.selector) await resolveLocator(page, step.selector).first().waitFor({ timeout });
      else if (step.urlPattern) await page.waitForURL(step.urlPattern, { timeout });
      else if (step.ms) await page.waitForTimeout(step.ms);
      return {};
    case "extract": {
      const value = await resolveLocator(page, step.selector)
        .first()
        .textContent({ timeout })
        .catch(() => null);
      // Captured into the run journal so later steps can reference it as
      // {{ steps.<id>.<name> }}. Null becomes "" rather than dropping the key,
      // so a downstream reference resolves to an empty capture instead of
      // failing as "never extracted" — the two are different diagnoses.
      return { [step.name]: (value ?? "").trim() };
    }
    case "verify":
    case "approval":
    case "apiCall":
    case "sendEmail":
      return {};
  }
}

/**
 * Re-apply a single step while rebuilding page state.
 *
 * Deliberately a *separate* switch from `applyStep` with no `click`,
 * `apiCall`, or `sendEmail` case. This is the structural fix for the old
 * `restoreBrowserPrefix`, which looped over `applyStep` and would happily
 * re-click an already-approved "Submit order" button. It is not possible to
 * route a mutating action through this function, because there is no code here
 * that performs one — and the exhaustiveness guard makes a future step type a
 * compile error rather than a silent replay.
 */
export async function applyRestorativeStep(
  page: Page,
  step: WorkflowStep,
  opts: StepOptions,
): Promise<void> {
  const timeout = opts.timeoutMs;

  switch (step.type) {
    case "fill":
      await resolveLocator(page, step.selector).first().fill(step.value, { timeout });
      return;
    case "select":
      await resolveLocator(page, step.selector).first().selectOption(step.value, { timeout });
      return;
    case "waitFor":
      if (step.selector) await resolveLocator(page, step.selector).first().waitFor({ timeout });
      else if (step.urlPattern) await page.waitForURL(step.urlPattern, { timeout });
      else if (step.ms) await page.waitForTimeout(step.ms);
      return;
    // `navigate` is subsumed by going straight to the captured URL.
    case "navigate":
    // Nothing to rebuild.
    case "verify":
    case "approval":
    case "extract":
      return;
    case "click":
    case "apiCall":
    case "sendEmail":
      throw new Error(
        `refusing to replay a ${step.type} step (${step.id}) while restoring page state: ` +
          "its effect may already have happened",
      );
  }
}

/**
 * Rebuild page state for a resumed run: go to the captured URL, then re-apply
 * only the steps the restore planner cleared as restorative *on that URL*.
 */
export async function applyRestorePlan(
  page: Page,
  steps: readonly WorkflowStep[],
  plan: Extract<RestorePlan, { kind: "restore" }>,
  opts: StepOptions,
): Promise<void> {
  await page.goto(plan.url, { waitUntil: "domcontentloaded", timeout: opts.timeoutMs });
  for (const index of plan.reapply) {
    const step = steps[index];
    if (step) await applyRestorativeStep(page, step, opts);
  }
}

/** Capture cookies + storage so a halted run can resume without replaying. */
export async function captureSessionState(
  context: BrowserContext,
  page: Page,
): Promise<{ state: Buffer; url: string }> {
  const state = await context.storageState();
  return { state: Buffer.from(JSON.stringify(state), "utf8"), url: page.url() };
}

/** Run just the verification half of a step, without re-running its action. */
export async function verifyStep(page: Page, step: WorkflowStep): Promise<VerifyResult | null> {
  if (step.type === "verify") return runVerification(page, step.assertion);
  if ("verify" in step && step.verify) return runVerification(page, step.verify);
  return null;
}

/**
 * Execute one workflow step against the page and capture a screenshot. Throws on
 * a failed browser action; the caller records the step as failed and stops.
 *
 * The action and the assertion are separable (`applyStep` / `verifyStep`) so the
 * shell can re-run a failed *verification* without re-running the action that
 * preceded it. Re-clicking "Submit" to find out whether the first click worked
 * is exactly the double-send this engine exists to prevent.
 */
/** True when capturing the page would retain secret-shaped pixels. */
export function shouldCaptureScreenshot(step: WorkflowStep): boolean {
  return !(step.type === "fill" && step.sensitive);
}

export async function runStep(
  page: Page,
  step: WorkflowStep,
  opts: StepOptions,
): Promise<StepResult> {
  const outputs = await applyStep(page, step, opts);
  const verification = await verifyStep(page, step);
  // Sensitive fills (OTP, card, password-shaped) leave secret pixels on the
  // page. Capturing them would put cardholder data into the artifact store
  // behind a positive allow-list of `step-*.png` keys. Skip the capture
  // entirely rather than store-and-redact — there is nothing safe to show.
  const screenshot = shouldCaptureScreenshot(step) ? await page.screenshot() : null;
  return { screenshot, verification, outputs, url: page.url() };
}
