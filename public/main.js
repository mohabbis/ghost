/* ============================================================
   Ghost marketing site — interactions.
   Desktop Organizer demos were removed when the product pivoted
   to the cloud AI operator; demo helpers below no-op if markup
   is absent so this file stays safe to load.
   ============================================================ */

const reduce = window.matchMedia("(prefers-reduced-motion: reduce)");

/* ---------- tiny helpers ---------- */
const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

function el(tag, className, html) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (html != null) node.innerHTML = html;
  return node;
}

// Run an array of callbacks spaced by `gap` ms (instant when reduced motion).
function sequence(steps, gap = 420) {
  if (reduce.matches) {
    steps.forEach((fn) => fn());
    return Promise.resolve();
  }
  return new Promise((resolve) => {
    steps.forEach((fn, i) => setTimeout(fn, i * gap));
    setTimeout(resolve, steps.length * gap);
  });
}

/* ---------- platform detection ---------- */
function platformDetection() {
  const ua = navigator.userAgent || "";
  const p = navigator.platform || "";
  let platform = "other";
  if (/Mac|iPhone|iPad|iPod/.test(ua) || /Mac/.test(p)) platform = "mac";
  else if (/Win/.test(ua) || /Win/.test(p)) platform = "windows";
  document.body.dataset.platform = platform;

  const sub = $("#download-sub");
  if (platform === "mac" && sub) {
    sub.textContent = "Detected macOS — legacy desktop v2.0.3 (notarized). Not the current product.";
  }
  if (platform === "windows" && sub) {
    sub.textContent = "Detected Windows — legacy desktop v2.0.3 (unsigned). Not the current product.";
  }

  const primaryLabel = $("[data-download-label]");
  if (primaryLabel) {
    if (platform === "mac") primaryLabel.textContent = "Legacy macOS v2.0.3";
    else if (platform === "windows") primaryLabel.textContent = "Legacy Windows v2.0.3";
  }
}

/* ---------- reveal on scroll ---------- */
function revealOnScroll() {
  const items = $$(".reveal");
  if (reduce.matches || !("IntersectionObserver" in window)) {
    items.forEach((n) => n.classList.add("is-visible"));
    return;
  }
  const io = new IntersectionObserver(
    (entries) => {
      entries.forEach((e) => {
        if (!e.isIntersecting) return;
        e.target.classList.add("is-visible");
        io.unobserve(e.target);
      });
    },
    { threshold: 0.12, rootMargin: "0px 0px -8% 0px" },
  );
  items.forEach((n) => io.observe(n));
}

/* ---------- ghost cursor-tracking eyes ---------- */
function setupGhostEyes() {
  if (reduce.matches) return;
  // Cursor tracking only makes sense with a real pointing device. On touch /
  // coarse-pointer devices the pointermove handler fires during scroll and
  // causes jank, so skip it entirely there.
  if (!window.matchMedia("(hover: hover) and (pointer: fine)").matches) return;
  const pupils = $$("[data-eye], .ghost-eye");
  if (!pupils.length) return;
  let raf = null;
  let mx = 0;
  let my = 0;

  const update = () => {
    raf = null;
    pupils.forEach((pupil) => {
      const r = pupil.getBoundingClientRect();
      const cx = r.left + r.width / 2;
      const cy = r.top + r.height / 2;
      const angle = Math.atan2(my - cy, mx - cx);
      const dist = Math.min(1.1, Math.hypot(mx - cx, my - cy) / 400);
      const x = Math.cos(angle) * dist;
      const y = Math.sin(angle) * dist;
      pupil.style.transform = `translate(${x}px, ${y}px)`;
    });
  };
  window.addEventListener(
    "pointermove",
    (e) => {
      mx = e.clientX;
      my = e.clientY;
      if (!raf) raf = requestAnimationFrame(update);
    },
    { passive: true },
  );
}

/* ---------- mobile nav ---------- */
function setupMobileNav() {
  const nav = $(".nav");
  const toggle = $("#nav-toggle");
  const links = $("#nav-links");
  if (!nav || !toggle || !links) return;
  const close = () => {
    nav.classList.remove("is-open");
    toggle.setAttribute("aria-expanded", "false");
  };
  toggle.addEventListener("click", () => {
    const open = nav.classList.toggle("is-open");
    toggle.setAttribute("aria-expanded", String(open));
  });
  links.addEventListener("click", (e) => {
    if (e.target.tagName === "A") close();
  });
  document.addEventListener("click", (e) => {
    if (nav.classList.contains("is-open") && !nav.contains(e.target)) close();
  });
  window.addEventListener("resize", () => {
    if (window.innerWidth > 900) close();
  });
}

/* ---------- demo tabs (WAI-ARIA: click + arrow-key navigation) ---------- */
function setupTabs() {
  const tabs = $$(".demo__tab");
  const panels = $$(".demo__panel");

  const select = (tab, focus = false) => {
    const name = tab.dataset.tab;
    tabs.forEach((t) => {
      const on = t === tab;
      t.classList.toggle("is-active", on);
      t.setAttribute("aria-selected", String(on));
      // Roving tabindex: only the active tab sits in the tab order.
      t.tabIndex = on ? 0 : -1;
    });
    panels.forEach((p) => {
      const on = p.dataset.panel === name;
      p.classList.toggle("is-active", on);
      p.hidden = !on;
    });
    if (focus) tab.focus();
  };

  tabs.forEach((tab, i) => {
    tab.tabIndex = tab.classList.contains("is-active") ? 0 : -1;
    tab.addEventListener("click", () => select(tab));
    tab.addEventListener("keydown", (e) => {
      let target = null;
      if (e.key === "ArrowRight" || e.key === "ArrowDown") target = tabs[(i + 1) % tabs.length];
      else if (e.key === "ArrowLeft" || e.key === "ArrowUp") target = tabs[(i - 1 + tabs.length) % tabs.length];
      else if (e.key === "Home") target = tabs[0];
      else if (e.key === "End") target = tabs[tabs.length - 1];
      if (target) {
        e.preventDefault();
        select(target, true);
      }
    });
  });
}

/* ---------- scrollspy: highlight the nav link for the section in view ---------- */
function setupScrollSpy() {
  const links = $$('.nav__links a[href^="#"]');
  if (!links.length || !("IntersectionObserver" in window)) return;
  const byId = new Map(links.map((a) => [a.getAttribute("href").slice(1), a]));
  const sections = [...byId.keys()].map((id) => document.getElementById(id)).filter(Boolean);
  const io = new IntersectionObserver(
    (entries) => {
      entries.forEach((e) => {
        const link = byId.get(e.target.id);
        if (!link) return;
        if (e.isIntersecting) {
          links.forEach((a) => a.classList.remove("is-active"));
          link.classList.add("is-active");
        }
      });
    },
    // A narrow band around the upper third of the viewport decides the
    // "current" section, so exactly one link is lit while scrolling.
    { rootMargin: "-20% 0px -70% 0px" },
  );
  sections.forEach((s) => io.observe(s));
}

/* ============================================================
   Demo A — Ghost Organizer trust pipeline
   ============================================================ */
function organizerDemo() {
  const filesEl = $("#org-files");
  const planEl = $("#org-plan");
  const auditBox = $("#org-audit");
  const auditEl = $("#org-auditlist");
  const btn = $("#org-action");
  const hint = $("#org-hint");
  const planHead = $("#org-planhead");
  if (!btn) return;

  const FILES = [
    { name: "receipt-final(2).pdf", meta: "PDF" },
    { name: "IMG_4821.HEIC", meta: "Image" },
    { name: "Q1 budget.xlsx", meta: "Sheet" },
    { name: "resume_v3.docx", meta: "Doc" },
    { name: "setup.dmg", meta: "Installer" },
    { name: "screenshot 2026-03-14.png", meta: "Image" },
  ];
  // Every action names the rule that authorized it and whether it was
  // automated or you-approved — the same "rule that fired / who signed off"
  // record the desktop app writes.
  const RULE = "~/Downloads";
  const PLAN = [
    { tag: "＋ folder", cls: "tag--new", strong: "Receipts", desc: "create", audit: "Created folder <b>Receipts</b>", prov: "you approved" },
    { tag: "→ move", cls: "tag--move", strong: "receipt-final(2).pdf", desc: "→ Receipts", file: 0, audit: "Moved <b>receipt-final(2).pdf</b> → Receipts", prov: "you approved" },
    { tag: "→ move", cls: "tag--move", strong: "IMG_4821.HEIC", desc: "→ Images", file: 1, audit: "Moved <b>IMG_4821.HEIC</b> → Images", prov: "you approved" },
    { tag: "→ move", cls: "tag--move", strong: "Q1 budget.xlsx", desc: "→ Documents/Spreadsheets", file: 2, audit: "Moved <b>Q1 budget.xlsx</b> → Documents/Spreadsheets", prov: "you approved" },
    { tag: "→ move", cls: "tag--move", strong: "screenshot 2026-03-14.png", desc: "→ Pictures/Screenshots", file: 5, audit: "Moved <b>screenshot 2026-03-14.png</b> → Pictures/Screenshots", prov: "you approved" },
    { tag: "⚠ conflict", cls: "tag--warn", strong: "setup.dmg", desc: "exists in Installers — keeps both, never overwrites", warn: true },
    { tag: "✕ denied", cls: "tag--deny", strong: "0 deletes · 0 uploads", desc: "outside your approval", deny: true },
  ];

  let state = "idle";

  function fileRow(f, i) {
    const li = el("li", null, `<span class="fname">${f.name}</span><span class="fmeta">${f.meta}</span>`);
    li.dataset.file = String(i);
    li.style.animationDelay = reduce.matches ? "0s" : `${i * 0.06}s`;
    return li;
  }
  function planRow(a) {
    const li = el(
      "li",
      a.warn ? "is-warn" : a.deny ? "is-deny" : null,
      `<span class="tag ${a.cls}">${a.tag}</span> <span class="p-strong">${a.strong}</span> <span class="p-desc">${a.desc}</span>`,
    );
    return li;
  }
  function auditRow(html, prov) {
    const rule = ` <span class="audit__rule">by rule <code>${RULE}</code></span>`;
    const badge = prov ? ` <span class="audit__prov">${prov}</span>` : "";
    return el("li", null, `✓ ${html}${badge}${rule}`);
  }

  function reset() {
    filesEl.innerHTML = "";
    planEl.innerHTML = "";
    auditEl.innerHTML = "";
    auditBox.hidden = true;
    planHead.textContent = "Proposed plan";
    hint.className = "demo__hint";
    hint.textContent = "Read-only scan — proposes changes, touches nothing.";
    btn.textContent = "Scan folder";
    btn.disabled = false;
    state = "idle";
  }

  async function scan() {
    btn.disabled = true;
    hint.textContent = "Scanning (read-only)…";
    FILES.forEach((f, i) => filesEl.appendChild(fileRow(f, i)));
    await sequence(
      PLAN.map((a) => () => planEl.appendChild(planRow(a))),
      260,
    );
    planHead.textContent = `Proposed plan — ${PLAN.filter((p) => !p.deny).length} actions`;
    hint.textContent = "Nothing changed yet. Review, then approve.";
    btn.textContent = "Approve & organize";
    btn.disabled = false;
    state = "planned";
  }

  async function execute() {
    btn.disabled = true;
    hint.textContent = "Executing your approved plan…";
    auditBox.hidden = false;
    const planRows = $$("li", planEl);
    const acts = PLAN.filter((a) => a.audit);
    await sequence(
      acts.map((a, i) => () => {
        planRows[i]?.classList.add("is-done");
        if (a.file != null) $(`li[data-file="${a.file}"]`, filesEl)?.classList.add("is-moved");
        auditEl.appendChild(auditRow(a.audit, a.prov));
      }),
      360,
    );
    hint.className = "demo__hint";
    hint.textContent = "Done — 5 changes, 0 deletes. Fully reversible.";
    btn.textContent = "↩︎ Undo everything";
    btn.disabled = false;
    state = "done";
  }

  async function undo() {
    btn.disabled = true;
    hint.textContent = "Reverting from the undo journal…";
    const acts = PLAN.filter((a) => a.audit);
    await sequence(
      acts
        .slice()
        .reverse()
        .map((a) => () => {
          if (a.file != null) $(`li[data-file="${a.file}"]`, filesEl)?.classList.remove("is-moved");
          auditEl.appendChild(auditRow(`Reverted: ${a.audit}`));
        }),
      300,
    );
    hint.textContent = "Restored to the original state. Nothing was lost.";
    btn.textContent = "Run it again";
    btn.disabled = false;
    state = "undone";
  }

  btn.addEventListener("click", () => {
    if (state === "idle") scan();
    else if (state === "planned") execute();
    else if (state === "done") undo();
    else if (state === "undone") reset();
  });

  reset();
}

/* ============================================================
   Demo B — Ghost 2.0 Action Plan (invoice → Finance)
   Faithful simulation of docs/GHOST_2_DEMO.md — not a live IPC call.
   ============================================================ */
function actionPlanDemo() {
  const planEl = $("#ap-plan");
  const receiptEl = $("#ap-receipt");
  const auditBox = $("#ap-audit");
  const auditEl = $("#ap-auditlist");
  const btn = $("#ap-action");
  const hint = $("#ap-hint");
  const title = $("#ap-title");
  const receiptHead = $("#ap-receipthead");
  if (!btn) return;

  // Mirrors action_plan_demo semantic steps: FS mutations + AX set_value log.
  // UI steps carry resolution_strategy evidence; undo notes stay honest.
  const STEPS = [
    {
      tag: "＋ folder",
      cls: "tag--new",
      strong: "Finance/Invoices",
      desc: "create",
      expected: "folder exists",
      observed: "created",
      strategy: null,
      undo: "journal entry written",
    },
    {
      tag: "✎ rename",
      cls: "tag--rename",
      strong: "invoice_jan.pdf",
      desc: "→ 2026-01-15_invoice_jan.pdf",
      expected: "dated name",
      observed: "renamed",
      strategy: null,
      undo: "journal entry written",
    },
    {
      tag: "→ move",
      cls: "tag--move",
      strong: "2026-01-15_invoice_jan.pdf",
      desc: "→ Finance/Invoices",
      expected: "file at destination",
      observed: "moved",
      strategy: null,
      undo: "journal entry written",
    },
    {
      tag: "↗ open",
      cls: "tag--new",
      strong: "TextEdit",
      desc: "launch app",
      expected: "app running",
      observed: "running",
      strategy: null,
      undo: "n/a: UI open is not reversible via the undo journal",
    },
    {
      tag: "◎ focus",
      cls: "tag--rename",
      strong: "AXTextArea",
      desc: "document body",
      expected: "focused",
      observed: "focused",
      strategy: "ax",
      undo: "n/a: UI click/type is not reversible via the undo journal",
    },
    {
      tag: "⌨ set",
      cls: "tag--move",
      strong: "log line",
      desc: "semantic set_value (not coordinate typing)",
      expected: "Filed invoice_jan.pdf",
      observed: "Filed invoice_jan.pdf",
      strategy: "ax",
      undo: "n/a: UI click/type is not reversible via the undo journal",
    },
    {
      tag: "⌘ save",
      cls: "tag--rename",
      strong: "Cmd+S",
      desc: "shortcut",
      expected: "document saved",
      observed: "saved",
      strategy: null,
      undo: "n/a: UI shortcut is not reversible via the undo journal",
    },
    {
      tag: "✓ verify",
      cls: "tag--new",
      strong: "path check",
      desc: "Finance/Invoices/… exists",
      expected: "path present",
      observed: "present",
      strategy: null,
      undo: null,
    },
  ];

  let state = "idle";

  function planRow(s) {
    const strategy = s.strategy
      ? `<span class="evidence-chip" title="UI resolution strategy">via ${s.strategy}</span>`
      : "";
    return el(
      "li",
      null,
      `<span class="tag ${s.cls}">${s.tag}</span> <span class="p-strong">${s.strong}</span> <span class="p-desc">${s.desc}</span>${strategy}`,
    );
  }

  function receiptRow(s) {
    const strategy = s.strategy
      ? `<span class="evidence-chip">via ${s.strategy}</span>`
      : "";
    return el(
      "li",
      "is-done",
      `<span class="tag tag--new">verified</span> <span class="p-strong">${s.strong}</span> <span class="p-desc">expected “${s.expected}” · observed “${s.observed}”</span>${strategy}`,
    );
  }

  function auditRow(html) {
    return el("li", null, html);
  }

  function reset() {
    planEl.innerHTML = "";
    receiptEl.innerHTML = "";
    auditEl.innerHTML = "";
    auditBox.hidden = true;
    title.textContent = "Zone · Downloads";
    receiptHead.textContent = "Receipt (awaiting approval)";
    hint.className = "demo__hint";
    hint.innerHTML =
      "Simulation of <code>action_plan_demo</code> — same approve → execute → verify shape as the desktop app.";
    btn.textContent = "Build invoice plan";
    btn.disabled = false;
    state = "idle";
  }

  async function buildPlan() {
    btn.disabled = true;
    hint.textContent = "Compiling a reviewable Action Plan — nothing executes yet.";
    title.textContent = "Action Plan · invoice → Finance";
    await sequence(
      STEPS.map((s) => () => planEl.appendChild(planRow(s))),
      220,
    );
    hint.className = "demo__hint is-warn";
    hint.textContent = "Deny-by-default. You approve the exact plan before Ghost runs a step.";
    btn.textContent = "Approve & execute";
    btn.disabled = false;
    state = "planned";
  }

  async function execute() {
    btn.disabled = true;
    hint.className = "demo__hint";
    hint.textContent = "Executing approved plan — verifying each step…";
    auditBox.hidden = false;
    receiptHead.textContent = "Execution receipt";
    const planRows = $$("li", planEl);
    await sequence(
      STEPS.map((s, i) => () => {
        planRows[i]?.classList.add("is-done");
        receiptEl.appendChild(receiptRow(s));
        if (s.undo) {
          auditEl.appendChild(
            auditRow(`✓ <b>${s.strong}</b> · undo: ${s.undo}`),
          );
        } else {
          auditEl.appendChild(auditRow(`✓ <b>${s.strong}</b> · verify-only`));
        }
      }),
      340,
    );
    hint.textContent =
      "Receipt sealed — FS steps reversible; UI steps recorded with honest undo notes. Per-step verify, not full source reconciliation.";
    btn.textContent = "↩︎ Undo FS steps";
    btn.disabled = false;
    state = "done";
  }

  async function undo() {
    btn.disabled = true;
    hint.textContent = "Replaying the undo journal for filesystem steps…";
    await sequence(
      [
        () => auditEl.appendChild(auditRow("↩ Reverted move of invoice")),
        () => auditEl.appendChild(auditRow("↩ Reverted rename")),
        () => auditEl.appendChild(auditRow("↩ Removed Finance/Invoices (if empty)")),
        () =>
          auditEl.appendChild(
            auditRow("· UI TextEdit steps left as-is (not journal-reversible)"),
          ),
      ],
      320,
    );
    hint.textContent = "Filesystem restored. UI log note remains — Ghost does not pretend clicks undo.";
    btn.textContent = "Run plan again";
    btn.disabled = false;
    state = "undone";
  }

  btn.addEventListener("click", () => {
    if (state === "idle") buildPlan();
    else if (state === "planned") execute();
    else if (state === "done") undo();
    else if (state === "undone") reset();
  });

  reset();
}

/* ============================================================
   Demo C — Record → Review → Replay → Verify → Undo
   ============================================================ */
function replayDemo() {
  const stepsEl = $("#rep-steps");
  const reviewEl = $("#rep-review");
  const auditBox = $("#rep-audit");
  const auditEl = $("#rep-auditlist");
  const btn = $("#rep-action");
  const hint = $("#rep-hint");
  const dot = $("#rec-dot");
  const title = $("#rec-title");
  if (!btn) return;

  // Flagship workflow: re-keying figures from a source into a close workbook.
  // Each step carries the value Ghost expects to see land in the field; on replay
  // it verifies observed-vs-expected and halts the moment one doesn't match.
  const STEPS = [
    { icon: "📄", html: 'Open <b>Q3 close — summary.xlsx</b>', expect: "workbook focused" },
    { icon: "⌨️", html: 'Set <b>B7 · Revenue</b> → <b>48,210.00</b>', expect: "48,210.00" },
    { icon: "⌨️", html: 'Set <b>B8 · COGS</b> → <b>12,900.00</b>', expect: "12,900.00", drift: "12,090.00" },
    { icon: "⌨️", html: 'Set <b>A1 · Period</b> → <b>2026-Q3</b>', expect: "2026-Q3" },
    { icon: "⏳", html: "Wait for recalculation (1.4s)", expect: "recalculated" },
  ];

  let state = "idle";

  function stepRow(s) {
    return el("li", null, `<span class="s-check">○</span> <span>${s.icon}</span> <span>${s.html}</span>`);
  }
  function auditRow(html) {
    return el("li", null, `✓ ${html}`);
  }
  function exceptionRow(html) {
    return el("li", "is-warn", `⚠ ${html}`);
  }

  // Index of the step whose source value drifted since recording — the one
  // Ghost is meant to catch on replay.
  const DRIFT = STEPS.findIndex((s) => s.drift);

  function reset() {
    stepsEl.innerHTML = "";
    reviewEl.innerHTML = "";
    auditEl.innerHTML = "";
    auditBox.hidden = true;
    dot.classList.remove("is-live");
    title.textContent = "Ready to record";
    hint.className = "demo__hint";
    hint.textContent = "Record the transfer once. On replay, Ghost checks every value against what you approved.";
    btn.textContent = "Record the transfer";
    btn.disabled = false;
    state = "idle";
  }

  async function record() {
    btn.disabled = true;
    dot.classList.add("is-live");
    title.textContent = "Recording…";
    hint.textContent = "Capturing each field and the exact value you enter.";
    await sequence(
      STEPS.map((s) => () => stepsEl.appendChild(stepRow(s))),
      420,
    );
    dot.classList.remove("is-live");
    title.textContent = `${STEPS.length} steps captured`;
    hint.textContent = "Review the readable steps before anything replays.";
    btn.textContent = "Review & approve";
    btn.disabled = false;
    state = "recorded";
  }

  async function review() {
    btn.disabled = true;
    await sequence(
      STEPS.map((s) => () => reviewEl.appendChild(stepRow(s))),
      160,
    );
    hint.className = "demo__hint is-warn";
    hint.textContent = "You approve every step and the value it should write. Deny-by-default until you say go.";
    btn.textContent = "Approve & replay";
    btn.disabled = false;
    state = "reviewed";
  }

  // Replay, verifying observed-vs-expected. Stops the run at the first mismatch.
  async function replay() {
    btn.disabled = true;
    hint.className = "demo__hint";
    hint.textContent = "Replaying — verifying every value as it lands…";
    auditBox.hidden = false;
    const rows = $$("li", stepsEl);
    for (let i = 0; i < STEPS.length; i++) {
      const s = STEPS[i];
      const check = $(".s-check", rows[i]);
      if (s.drift) {
        rows[i]?.classList.add("is-warn");
        if (check) check.textContent = "⚠";
        auditEl.appendChild(
          exceptionRow(
            `Exception at <b>B8 · COGS</b> — approved <b>${s.expect}</b>, source now reads <b>${s.drift}</b>. Run halted before writing.`,
          ),
        );
        hint.className = "demo__hint is-warn";
        hint.textContent = "Caught a mismatch — Ghost stopped before writing a wrong number. Nothing downstream changed.";
        btn.textContent = "Review exception →";
        btn.disabled = false;
        state = "halted";
        return;
      }
      rows[i]?.classList.add("is-done");
      if (check) check.textContent = "●";
      auditEl.appendChild(auditRow(`Verified — ${s.html.replace(/<[^>]+>/g, "").trim()} · reads “${s.expect}”`));
      await sequence([() => {}], 380);
    }
  }

  async function resolveException() {
    btn.disabled = true;
    hint.className = "demo__hint";
    hint.textContent = "You reconciled the exception. Finishing the remaining approved steps…";
    const rows = $$("li", stepsEl);
    for (let i = DRIFT; i < STEPS.length; i++) {
      const s = STEPS[i];
      const check = $(".s-check", rows[i]);
      rows[i]?.classList.remove("is-warn");
      rows[i]?.classList.add("is-done");
      if (check) check.textContent = "●";
      auditEl.appendChild(auditRow(`Verified — ${s.html.replace(/<[^>]+>/g, "").trim()} · reads “${s.expect}”`));
      await sequence([() => {}], 360);
    }
    hint.textContent = "Run complete — every value verified, receipt sealed, and reversible.";
    btn.textContent = "↩︎ Undo run";
    btn.disabled = false;
    state = "replayed";
  }

  async function undo() {
    btn.disabled = true;
    hint.textContent = "Reverting from the undo journal…";
    auditEl.appendChild(auditRow("Reverted: every written cell restored to its prior value"));
    await sequence([() => {}], 500);
    hint.textContent = "Back to where you started. Nothing was lost.";
    btn.textContent = "Record again";
    btn.disabled = false;
    state = "undone";
  }

  btn.addEventListener("click", () => {
    if (state === "idle") record();
    else if (state === "recorded") review();
    else if (state === "reviewed") replay();
    else if (state === "halted") resolveException();
    else if (state === "replayed") undo();
    else if (state === "undone") reset();
  });

  reset();
}

/* ---------- nav hairline appears once the page scrolls ---------- */
function setupStickyNav() {
  const nav = $(".nav");
  if (!nav) return;
  const onScroll = () => nav.classList.toggle("is-stuck", window.scrollY > 8);
  onScroll();
  window.addEventListener("scroll", onScroll, { passive: true });
}

/* ---------- boot ---------- */
window.addEventListener("DOMContentLoaded", () => {
  platformDetection();
  revealOnScroll();
  setupGhostEyes();
  setupStickyNav();
  setupMobileNav();
  setupTabs();
  setupScrollSpy();
  // Legacy desktop demos — only if old markup is present.
  if ($("[data-panel='organizer']")) organizerDemo();
  if ($("#ap-run")) actionPlanDemo();
  if ($("#rep-run")) replayDemo();
});
