import type { Page, Locator } from "playwright";
import type { Selector } from "@ghost/core/schema/step";

type Role = Parameters<Page["getByRole"]>[0];

/**
 * Resolve a Ghost `Selector` to a Playwright `Locator`, preferring semantic
 * strategies over CSS. Order mirrors the desktop resolution chain: accessible
 * role+name first, then test id, then visible text, then raw CSS as the last
 * resort. Coordinates are never used.
 */
export function resolveLocator(page: Page, sel: Selector): Locator {
  if (sel.role && sel.name) return page.getByRole(sel.role as Role, { name: sel.name });
  if (sel.testId) return page.getByTestId(sel.testId);
  if (sel.text) return page.getByText(sel.text);
  if (sel.css) return page.locator(sel.css);
  if (sel.role) return page.getByRole(sel.role as Role);
  if (sel.name) return page.getByText(sel.name);
  throw new Error("selector has no resolvable field (role/name/testId/text/css)");
}
