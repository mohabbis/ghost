/**
 * The page-side recorder, as a function that can be shipped into a page.
 *
 * This is the same job `apps/extension/src/content.js` does for the Chrome
 * extension, written so a Playwright context can inject it with
 * `addInitScript` — which is how the remote cloud browser records. The
 * extension keeps its own copy because it is loaded as a raw content script
 * with no build step; `recorder.test.ts` pins the security-critical constants
 * of the two together so they cannot drift apart silently.
 *
 * `installRecorder` is serialised with `Function.prototype.toString()`, so it
 * must be **completely self-contained**: no imports, no module-scope
 * references, no closures. Nothing outside this function body exists in the
 * page. TypeScript annotations are erased before serialisation and are safe;
 * anything that compiles to a runtime helper (enums, decorators, `class`
 * fields with initialisers that lower) is not.
 *
 * The two rules from the extension hold here identically, and for the same
 * reasons:
 *
 * 1. **A secret never enters the trace.** Not encrypted, not truncated —
 *    absent. Redaction happens at capture because it is the only place it can
 *    be done honestly. `redacted: true` records that something was typed
 *    without recording what.
 * 2. **No coordinates, ever.** The user drives the remote browser with a
 *    mouse, so coordinates cross the socket — but they stop at the page. What
 *    is recorded is the accessible role and name of whatever the pointer
 *    landed on, read off the live element while it is still on screen. An
 *    element that cannot be named is reported as unidentifiable rather than
 *    approximated by its position.
 */

// --- browser globals, declared locally ------------------------------------
//
// This is the only browser file in a package of server code, and TypeScript's
// `lib` setting has no per-file scope: adding `dom` — in a tsconfig or via a
// `/// <reference lib="dom" />` — pulls `lib.dom.d.ts` into the whole program.
// That is not a style preference. It re-typed `ReadableStream` in
// `src/storage/artifacts.ts` to the browser's, breaking a file that reads
// Vercel Blob streams and has nothing to do with any of this, and it would
// hand every other file here a `document` it must never touch.
//
// So the handful of globals this recorder actually uses are declared here
// instead. `declare` inside a module is module-scoped, so nothing escapes the
// file, and the surface is small enough to be worth the honesty: what follows
// is exactly the browser API the recorder depends on.

interface RecorderElement {
  readonly tagName: string;
  readonly id: string;
  /** 1 for an element; guards against text and comment nodes. */
  readonly nodeType: number;
  readonly textContent: string | null;
  readonly parentElement: RecorderElement | null;
  readonly children: ArrayLike<RecorderElement>;
  getAttribute(name: string): string | null;
  hasAttribute(name: string): boolean;
  closest(selectors: string): RecorderElement | null;
}

/** Both a type and a value: `instanceof Element` needs the constructor. */
type Element = RecorderElement;
declare const Element: { prototype: RecorderElement; new (): RecorderElement };

/** Only `.value` is read off these, and never on a secret field. */
type HTMLInputElement = RecorderElement & { value: string };
type HTMLSelectElement = RecorderElement & { value: string };

declare const document: {
  getElementById(id: string): RecorderElement | null;
  querySelector(selectors: string): RecorderElement | null;
  addEventListener(
    type: string,
    listener: (event: { target: unknown }) => void,
    capture?: boolean,
  ): void;
};

declare const window: {
  readonly top: unknown;
  readonly self: unknown;
  readonly history: unknown;
  addEventListener(type: string, listener: () => void): void;
};

declare const location: { readonly href: string };

/** `CSS.escape`, so an id with a colon in it cannot break out of a selector. */
declare const CSS: { escape(value: string): string };

/** Name of the Playwright binding the recorder emits through. */
export const CAPTURE_BINDING = "__ghostCaptureEmit";

/**
 * Installs the recorder in the current page.
 *
 * Exported for the type-checker and for direct use in tests; production
 * injection goes through `recorderInitScript`.
 */
export function installRecorder(bindingName: string): void {
  const w = window as unknown as Record<string, unknown>;
  if (w.__ghostRecorderInstalled) return;
  // Only the top frame. A selector carries no frame identity, so a step
  // compiled from an iframe's DOM would resolve against the wrong document on
  // replay — worse than not recording it.
  if (window.top !== window.self) return;
  w.__ghostRecorderInstalled = true;

  const pending: unknown[] = [];

  function flush(): void {
    const emit = w[bindingName];
    if (typeof emit !== "function") return;
    while (pending.length > 0) {
      const event = pending.shift();
      try {
        (emit as (arg: unknown) => unknown)(event);
      } catch {
        // The binding disappears during navigation teardown. Losing the last
        // event of a page is better than throwing inside a page's own handler,
        // which would break the site the user is demonstrating.
        return;
      }
    }
  }

  function send(event: unknown): void {
    pending.push(event);
    flush();
  }

  // --- sensitivity -------------------------------------------------------

  const SECRET_HINTS =
    /pass(word|code)|otp|one[-_ ]?time|2fa|mfa|cvv|cvc|card[-_ ]?number|credit|account[-_ ]?number|routing|ssn|social[-_ ]?security|\bpin\b|secret|token|security[-_ ]?code/i;

  const SECRET_AUTOCOMPLETE = /current-password|new-password|one-time-code|cc-number|cc-csc|cc-exp/i;

  function isSecretField(el: Element): boolean {
    if (!el) return false;
    const type = (el.getAttribute("type") || "").toLowerCase();
    if (type === "password") return true;
    if (SECRET_AUTOCOMPLETE.test(el.getAttribute("autocomplete") || "")) return true;

    const haystack = [
      el.getAttribute("name"),
      el.getAttribute("id"),
      el.getAttribute("placeholder"),
      el.getAttribute("aria-label"),
      accessibleName(el),
    ]
      .filter(Boolean)
      .join(" ");
    return SECRET_HINTS.test(haystack);
  }

  function isIgnorableField(el: Element): boolean {
    return (el.getAttribute("type") || "").toLowerCase() === "hidden";
  }

  // --- accessible role and name -----------------------------------------

  function implicitRole(el: Element): string | undefined {
    const tag = el.tagName.toLowerCase();
    const type = (el.getAttribute("type") || "").toLowerCase();
    if (tag === "a" && el.hasAttribute("href")) return "link";
    if (tag === "button") return "button";
    if (tag === "select") return "combobox";
    if (tag === "textarea") return "textbox";
    if (tag === "input") {
      if (["submit", "button", "reset", "image"].indexOf(type) !== -1) return "button";
      if (type === "checkbox") return "checkbox";
      if (type === "radio") return "radio";
      if (["text", "email", "tel", "url", "search", "password", "number", ""].indexOf(type) !== -1) {
        return "textbox";
      }
    }
    if (/^h[1-6]$/.test(tag)) return "heading";
    return undefined;
  }

  function roleOf(el: Element): string | undefined {
    return el.getAttribute("role") || implicitRole(el);
  }

  function textOf(el: Element | null): string {
    return ((el && el.textContent) || "").replace(/\s+/g, " ").trim();
  }

  function accessibleName(el: Element): string {
    if (!el || el.nodeType !== 1) return "";

    const labelledBy = el.getAttribute("aria-labelledby");
    if (labelledBy) {
      const parts = labelledBy
        .split(/\s+/)
        .map((id) => document.getElementById(id))
        .filter(Boolean)
        .map(textOf)
        .filter(Boolean);
      if (parts.length) return parts.join(" ");
    }

    const ariaLabel = el.getAttribute("aria-label");
    if (ariaLabel && ariaLabel.trim()) return ariaLabel.trim();

    if (el.id) {
      const label = document.querySelector('label[for="' + CSS.escape(el.id) + '"]');
      if (label) {
        const t = textOf(label);
        if (t) return t;
      }
    }

    const wrapping = el.closest("label");
    if (wrapping) {
      const t = textOf(wrapping);
      if (t) return t;
    }

    const tag = el.tagName.toLowerCase();
    if (tag === "input") {
      const type = (el.getAttribute("type") || "").toLowerCase();
      if (["submit", "button", "reset"].indexOf(type) !== -1) {
        const v = el.getAttribute("value");
        if (v && v.trim()) return v.trim();
      }
      const placeholder = el.getAttribute("placeholder");
      if (placeholder && placeholder.trim()) return placeholder.trim();
    }

    if (tag === "img") {
      const alt = el.getAttribute("alt");
      if (alt && alt.trim()) return alt.trim();
    }

    const title = el.getAttribute("title");
    if (title && title.trim()) return title.trim();

    if (
      ["button", "a", "summary"].indexOf(tag) !== -1 ||
      el.getAttribute("role") === "button"
    ) {
      const t = textOf(el);
      if (t) return t.slice(0, 200);
    }

    return "";
  }

  // --- selector candidates ----------------------------------------------

  function cssPath(el: Element): string {
    const parts: string[] = [];
    let node: Element | null = el;
    let depth = 0;
    while (node && node.nodeType === 1 && depth < 5) {
      let part = node.tagName.toLowerCase();
      if (node.id) {
        parts.unshift("#" + CSS.escape(node.id));
        break;
      }
      const parent: Element | null = node.parentElement;
      if (parent) {
        const current: Element = node;
        const siblings = Array.prototype.slice
          .call(parent.children)
          .filter((c: Element) => c.tagName === current.tagName);
        if (siblings.length > 1) part += ":nth-of-type(" + (siblings.indexOf(current) + 1) + ")";
      }
      parts.unshift(part);
      node = node.parentElement;
      depth++;
    }
    return parts.join(" > ");
  }

  function testIdOf(el: Element): string | undefined {
    return (
      el.getAttribute("data-testid") ||
      el.getAttribute("data-test-id") ||
      el.getAttribute("data-test") ||
      undefined
    );
  }

  function selectorCandidates(el: Element): string[] {
    const out: string[] = [];
    const testId = testIdOf(el);
    if (testId) out.push('[data-testid="' + CSS.escape(testId) + '"]');
    if (el.id) out.push("#" + CSS.escape(el.id));
    const name = el.getAttribute("name");
    if (name) out.push(el.tagName.toLowerCase() + '[name="' + CSS.escape(name) + '"]');
    const path = cssPath(el);
    if (path) out.push(path);
    return out.slice(0, 5);
  }

  function describeTarget(el: Element): Record<string, unknown> {
    return {
      role: roleOf(el) || undefined,
      name: accessibleName(el) || undefined,
      testId: testIdOf(el),
      text: textOf(el).slice(0, 120) || undefined,
      selectorCandidates: selectorCandidates(el),
      tagName: el.tagName.toLowerCase(),
      inputType: (el.getAttribute("type") || "").toLowerCase() || undefined,
    };
  }

  const now = (): number => Date.now();

  // --- listeners ---------------------------------------------------------

  // Capture phase: a page that calls stopPropagation in its own handlers would
  // otherwise make the workflow unrecordable.
  document.addEventListener(
    "click",
    (e) => {
      const raw = e.target;
      const el =
        raw instanceof Element
          ? raw.closest("a,button,[role],input,summary,label") || raw
          : null;
      if (!el || el.nodeType !== 1) return;
      if (isIgnorableField(el)) return;
      send({ type: "click", url: location.href, timestamp: now(), target: describeTarget(el) });
    },
    true,
  );

  document.addEventListener(
    "change",
    (e) => {
      const el = e.target;
      if (!(el instanceof Element) || isIgnorableField(el)) return;
      const tag = el.tagName.toLowerCase();

      if (tag === "select") {
        send({
          type: "select",
          url: location.href,
          timestamp: now(),
          target: describeTarget(el),
          value: (el as HTMLSelectElement).value || "",
        });
        return;
      }

      if (tag === "input" || tag === "textarea") {
        const type = (el.getAttribute("type") || "").toLowerCase();
        if (["checkbox", "radio", "submit", "button", "file"].indexOf(type) !== -1) return;

        const secret = isSecretField(el);
        const event: Record<string, unknown> = {
          type: "input",
          url: location.href,
          timestamp: now(),
          target: describeTarget(el),
          redacted: secret,
        };
        // The whole point: on a secret field the value is not read at all.
        if (!secret) event.value = (el as HTMLInputElement).value || "";
        send(event);
      }
    },
    true,
  );

  document.addEventListener(
    "submit",
    (e) => {
      const el = e.target instanceof Element ? e.target : null;
      const event: Record<string, unknown> = {
        type: "submit",
        url: location.href,
        timestamp: now(),
      };
      if (el) event.target = describeTarget(el);
      send(event);
    },
    true,
  );

  // SPA navigation: pushState/replaceState fire no event of their own.
  let lastUrl = location.href;
  const reportNavigation = (): void => {
    if (location.href === lastUrl) return;
    lastUrl = location.href;
    send({ type: "navigate", url: location.href, timestamp: now() });
  };

  const history = window.history as unknown as Record<string, unknown>;
  for (const method of ["pushState", "replaceState"]) {
    const original = history[method] as (...args: unknown[]) => unknown;
    history[method] = function patched(this: unknown, ...args: unknown[]): unknown {
      const result = original.apply(this, args);
      reportNavigation();
      return result;
    };
  }
  window.addEventListener("popstate", reportNavigation);
  window.addEventListener("hashchange", reportNavigation);

  // The page this document loaded on, so the compiled workflow opens somewhere.
  // Emitted on every document, not just the first: a full page load creates a
  // new document and re-runs this script, and that transition is exactly the
  // `navigate` step a replay needs.
  send({ type: "navigate", url: location.href, timestamp: now() });

  // The binding may be installed after this script runs. Retry the queue on the
  // next tick and once the document is interactive rather than dropping the
  // opening navigate.
  setTimeout(flush, 0);
  document.addEventListener("DOMContentLoaded", flush);
}

/**
 * The recorder as a source string, ready for `context.addInitScript`.
 *
 * Serialising the function rather than keeping a parallel string literal means
 * the injected code is the code above — type-checked, lint-covered, and
 * reviewable — instead of a blob nothing checks.
 */
export function recorderInitScript(bindingName: string = CAPTURE_BINDING): string {
  return `(${installRecorder.toString()})(${JSON.stringify(bindingName)});`;
}
