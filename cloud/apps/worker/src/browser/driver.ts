import { chromium, type Browser, type BrowserContext, type Page } from "playwright";
import type { WorkflowStep } from "@ghost/core/schema/step";
import { resolveLocator } from "./selector.js";
import { runVerification, type VerifyResult } from "./verify.js";

export interface StepResult {
  screenshot: Buffer;
  verification: VerifyResult | null;
}

function launchOptions() {
  // In this repo's environments Chromium is preinstalled; allow an explicit
  // executable path so we never trigger a download or hit a revision mismatch.
  const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE || undefined;
  return { headless: true, executablePath, args: ["--no-sandbox"] };
}

/**
 * A single browser session for one run. Phase 1 uses an ephemeral context per
 * run; a persistent per-org context (for stored cookies/credentials) arrives
 * with recording in Phase 2.
 */
export class BrowserSession {
  private constructor(
    private readonly browser: Browser,
    readonly context: BrowserContext,
    readonly page: Page,
  ) {}

  static async launch(): Promise<BrowserSession> {
    const browser = await chromium.launch(launchOptions());
    const context = await browser.newContext();
    const page = await context.newPage();
    return new BrowserSession(browser, context, page);
  }

  async close(): Promise<void> {
    await this.browser.close();
  }
}

/**
 * Apply one workflow step's browser action (no screenshot / verification).
 * `approval` and connector steps (`apiCall`/`sendEmail`) do not act on the page
 * in Phase 1 — the approval gate is handled by the state machine, and connector
 * execution lands in a later phase.
 */
export async function applyStep(page: Page, step: WorkflowStep): Promise<void> {
  switch (step.type) {
    case "navigate":
      await page.goto(step.url, { waitUntil: "domcontentloaded" });
      break;
    case "click":
      await resolveLocator(page, step.selector).first().click();
      break;
    case "fill":
      await resolveLocator(page, step.selector).first().fill(step.value);
      break;
    case "select":
      await resolveLocator(page, step.selector).first().selectOption(step.value);
      break;
    case "waitFor":
      if (step.selector) await resolveLocator(page, step.selector).first().waitFor();
      else if (step.urlPattern) await page.waitForURL(step.urlPattern);
      else if (step.ms) await page.waitForTimeout(step.ms);
      break;
    case "extract":
      // Read-only in Phase 1; a variable store for reusing extracted values
      // arrives with the step editor.
      await resolveLocator(page, step.selector)
        .first()
        .textContent()
        .catch(() => null);
      break;
    case "verify":
    case "approval":
    case "apiCall":
    case "sendEmail":
      break;
  }
}

/**
 * Rebuild page state after an approval halt.
 *
 * Phase 1 uses an ephemeral browser per job: when the worker stops at a gate it
 * closes Chromium, so resume would otherwise start on `about:blank` and fail to
 * find the gated control. Replaying the prefix (`steps[0..cursor)`) restores the
 * page. Correctly authored workflows gate *before* the irreversible action, so
 * the prefix is setup (navigate/fill), not a double-submit. Phase 2 persistent
 * contexts will replace this.
 */
export async function restoreBrowserPrefix(
  page: Page,
  prefix: readonly WorkflowStep[],
): Promise<void> {
  for (const step of prefix) {
    await applyStep(page, step);
  }
}

/**
 * Execute one workflow step against the page and capture a screenshot. Throws on
 * a failed browser action; the caller records the step as failed and stops.
 */
export async function runStep(page: Page, step: WorkflowStep): Promise<StepResult> {
  await applyStep(page, step);

  const verification =
    step.type === "verify"
      ? await runVerification(page, step.assertion)
      : "verify" in step && step.verify
        ? await runVerification(page, step.verify)
        : null;

  const screenshot = await page.screenshot();
  return { screenshot, verification };
}
