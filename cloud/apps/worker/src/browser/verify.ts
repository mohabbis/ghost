import type { Page } from "playwright";
import type { Verification } from "@ghost/core/schema/step";
import { resolveLocator } from "./selector.js";

export interface VerifyResult {
  passed: boolean;
  detail: string;
}

/**
 * Run a step's verification assertion against the live page. Deterministic and
 * side-effect free — this is MVP pillar 5's per-step "did the outcome actually
 * happen" check, returned as a structured result the run log stores.
 */
export async function runVerification(page: Page, v: Verification): Promise<VerifyResult> {
  switch (v.kind) {
    case "url": {
      const current = page.url();
      const passed = current === v.expected || current.startsWith(v.expected);
      return { passed, detail: `url=${current} expected=${v.expected}` };
    }
    case "selectorVisible": {
      const visible = await resolveLocator(page, v.selector)
        .first()
        .isVisible()
        .catch(() => false);
      return { passed: visible, detail: visible ? "visible" : "not visible" };
    }
    case "textPresent": {
      const body = (await page.textContent("body").catch(() => "")) ?? "";
      const passed = body.includes(v.expected);
      return { passed, detail: passed ? `found "${v.expected}"` : `missing "${v.expected}"` };
    }
  }
}
