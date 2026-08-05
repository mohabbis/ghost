/**
 * Ghost recorder — page-side capture.
 *
 * Emits the trace format in `@ghost/core/recording/trace`, whose defining
 * property is that **accessible role and name are read here**, off the live
 * element while it is still on screen. Those are the fields the worker's
 * resolution chain prefers, so the trace compiles into steps deterministically
 * with no model in the correctness path. Inferring them later from a HAR is
 * guesswork; reading them now is not.
 *
 * Two rules this file must not break:
 *
 * 1. **A secret never enters the trace.** Not encrypted, not truncated —
 *    absent. Redaction happens at capture because it is the only place it can
 *    be done honestly: a trace carrying the value plus a "please ignore"
 *    flag has already leaked it. `redacted: true` records that something was
 *    typed without recording what.
 * 2. **No coordinates, ever.** There is no step field for them, and a
 *    coordinate-replayed click is the failure mode the whole semantic
 *    selector chain exists to avoid. An element we cannot name is reported as
 *    unidentifiable rather than approximated.
 *
 * Runs in the top frame only (`all_frames: false`). Cross-origin iframe
 * contents are deliberately out of scope.
 */

(() => {
  if (window.__ghostRecorderInstalled) return;
  window.__ghostRecorderInstalled = true;

  let recording = false;

  chrome.storage.local.get(["recording"], (state) => {
    recording = Boolean(state.recording);
  });

  chrome.storage.onChanged.addListener((changes, area) => {
    if (area === "local" && changes.recording) recording = Boolean(changes.recording.newValue);
  });

  const send = (event) => {
    if (!recording) return;
    try {
      chrome.runtime.sendMessage({ kind: "ghost-event", event });
    } catch {
      // The service worker may be asleep or the extension reloading. Dropping
      // one event is better than throwing inside a page's event handler.
    }
  };

  // --- sensitivity -------------------------------------------------------

  const SECRET_HINTS =
    /pass(word|code)|otp|one[-_ ]?time|2fa|mfa|cvv|cvc|card[-_ ]?number|credit|account[-_ ]?number|routing|ssn|social[-_ ]?security|\bpin\b|secret|token|security[-_ ]?code/i;

  const SECRET_AUTOCOMPLETE =
    /current-password|new-password|one-time-code|cc-number|cc-csc|cc-exp/i;

  /** Whether this field's *value* must never be captured. */
  function isSecretField(el) {
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

  /** Fields that are never recorded at all, value or otherwise. */
  function isIgnorableField(el) {
    const type = (el.getAttribute("type") || "").toLowerCase();
    return type === "hidden";
  }

  // --- accessible role and name -----------------------------------------

  function implicitRole(el) {
    const tag = el.tagName.toLowerCase();
    const type = (el.getAttribute("type") || "").toLowerCase();
    if (tag === "a" && el.hasAttribute("href")) return "link";
    if (tag === "button") return "button";
    if (tag === "select") return "combobox";
    if (tag === "textarea") return "textbox";
    if (tag === "input") {
      if (["submit", "button", "reset", "image"].includes(type)) return "button";
      if (type === "checkbox") return "checkbox";
      if (type === "radio") return "radio";
      if (["text", "email", "tel", "url", "search", "password", "number", ""].includes(type)) {
        return "textbox";
      }
    }
    if (/^h[1-6]$/.test(tag)) return "heading";
    return undefined;
  }

  function role(el) {
    return el.getAttribute("role") || implicitRole(el);
  }

  function textOf(el) {
    return (el.textContent || "").replace(/\s+/g, " ").trim();
  }

  /**
   * Accessible name, in roughly the order the accname spec resolves it.
   *
   * Not a complete implementation — that is a large specification — but it
   * covers the attributes real applications actually use, and it is computed
   * from the same DOM Playwright's `getByRole(role, { name })` will later
   * query, which is what matters for replay.
   */
  function accessibleName(el) {
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
      const label = document.querySelector(`label[for="${CSS.escape(el.id)}"]`);
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
      if (["submit", "button", "reset"].includes(type)) {
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

    // Buttons and links are named by their own text.
    if (["button", "a", "summary"].includes(tag) || el.getAttribute("role") === "button") {
      const t = textOf(el);
      if (t) return t.slice(0, 200);
    }

    return "";
  }

  // --- selector candidates ----------------------------------------------

  function cssPath(el) {
    const parts = [];
    let node = el;
    let depth = 0;
    while (node && node.nodeType === 1 && depth < 5) {
      let part = node.tagName.toLowerCase();
      if (node.id) {
        parts.unshift(`#${CSS.escape(node.id)}`);
        break;
      }
      const parent = node.parentElement;
      if (parent) {
        const siblings = Array.from(parent.children).filter((c) => c.tagName === node.tagName);
        if (siblings.length > 1) part += `:nth-of-type(${siblings.indexOf(node) + 1})`;
      }
      parts.unshift(part);
      node = node.parentElement;
      depth++;
    }
    return parts.join(" > ");
  }

  /**
   * Ordered best-first CSS fallbacks. Several, not one, because the worker
   * replays in a different browser and a single brittle selector is the
   * failure that invites.
   */
  function selectorCandidates(el) {
    const out = [];
    const testId =
      el.getAttribute("data-testid") ||
      el.getAttribute("data-test-id") ||
      el.getAttribute("data-test");
    if (testId) out.push(`[data-testid="${CSS.escape(testId)}"]`);
    if (el.id) out.push(`#${CSS.escape(el.id)}`);
    const name = el.getAttribute("name");
    if (name) out.push(`${el.tagName.toLowerCase()}[name="${CSS.escape(name)}"]`);
    const path = cssPath(el);
    if (path) out.push(path);
    return out.slice(0, 5);
  }

  function describeTarget(el) {
    return {
      role: role(el) || undefined,
      name: accessibleName(el) || undefined,
      testId:
        el.getAttribute("data-testid") ||
        el.getAttribute("data-test-id") ||
        el.getAttribute("data-test") ||
        undefined,
      text: textOf(el).slice(0, 120) || undefined,
      selectorCandidates: selectorCandidates(el),
      tagName: el.tagName.toLowerCase(),
      inputType: (el.getAttribute("type") || "").toLowerCase() || undefined,
    };
  }

  const now = () => Date.now();

  // --- listeners ---------------------------------------------------------

  // Capture phase: a page that calls stopPropagation on its own handlers
  // would otherwise make the workflow unrecordable.
  document.addEventListener(
    "click",
    (e) => {
      const el = e.target instanceof Element ? e.target.closest("a,button,[role],input,summary,label") || e.target : null;
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
          value: el.value ?? "",
        });
        return;
      }

      if (tag === "input" || tag === "textarea") {
        const type = (el.getAttribute("type") || "").toLowerCase();
        if (["checkbox", "radio", "submit", "button", "file"].includes(type)) return;

        const secret = isSecretField(el);
        send({
          type: "input",
          url: location.href,
          timestamp: now(),
          target: describeTarget(el),
          // The whole point: on a secret field the value is not read at all.
          ...(secret ? {} : { value: el.value ?? "" }),
          redacted: secret,
        });
      }
    },
    true,
  );

  document.addEventListener(
    "submit",
    (e) => {
      const el = e.target instanceof Element ? e.target : null;
      send({
        type: "submit",
        url: location.href,
        timestamp: now(),
        ...(el ? { target: describeTarget(el) } : {}),
      });
    },
    true,
  );

  // SPA navigation: pushState/replaceState fire no event of their own.
  let lastUrl = location.href;
  const reportNavigation = () => {
    if (location.href === lastUrl) return;
    lastUrl = location.href;
    send({ type: "navigate", url: location.href, timestamp: now() });
  };

  for (const method of ["pushState", "replaceState"]) {
    const original = history[method];
    history[method] = function patched(...args) {
      const result = original.apply(this, args);
      reportNavigation();
      return result;
    };
  }
  window.addEventListener("popstate", reportNavigation);
  window.addEventListener("hashchange", reportNavigation);

  // The initial page of a recording, so the compiled workflow opens somewhere.
  send({ type: "navigate", url: location.href, timestamp: now() });
})();
