/* ============================================================
   Ghost marketing site — interactions + interactive demos.
   Vanilla ES module, no dependencies. All demos are faithful
   in-browser SIMULATIONS of the desktop app; nothing here
   touches real files or the network.
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
  if (platform === "mac" && sub) sub.textContent = "Detected macOS — v2.0.2 (notarized).";
  if (platform === "windows" && sub) sub.textContent = "Detected Windows — v2.0.2 (unsigned installer).";

  const primaryLabel = $("[data-download-label]");
  if (primaryLabel) {
    if (platform === "mac") primaryLabel.textContent = "Download v2.0.2 for macOS";
    else if (platform === "windows") primaryLabel.textContent = "Download v2.0.2 for Windows";
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
   Demo B — Plan Filing (preview only; Organizer executes)
   ============================================================ */
function planFilingDemo() {
  const filesEl = $("#filing-files");
  const planEl = $("#filing-plan");
  const auditBox = $("#filing-audit");
  const auditEl = $("#filing-auditlist");
  const btn = $("#filing-action");
  const hint = $("#filing-hint");
  const planHead = $("#filing-planhead");
  if (!btn) return;

  // Mirrors the desktop Plan Filing view: paste names → preview by profile.
  // Nothing here mutates disk; Organizer is where approve / audit / undo live.
  const FILES = [
    { name: "junit-test-results-2026-07.xml", meta: "Test report" },
    { name: "coverage-Q2-2026.html", meta: "Coverage" },
    { name: "build-log-2026-07-12.txt", meta: "Build log" },
    { name: "screenshot-flaky-login.png", meta: "Screenshot" },
    { name: "trace-run-8842.zip", meta: "Trace" },
  ];
  const PLAN = [
    { tag: "＋ folder", cls: "tag--new", strong: "Test reports", desc: "by artifact type" },
    { tag: "＋ folder", cls: "tag--new", strong: "Coverage", desc: "by reporting period" },
    { tag: "＋ folder", cls: "tag--new", strong: "Build logs", desc: "by run date" },
    { tag: "→ file", cls: "tag--move", strong: "junit-test-results-2026-07.xml", desc: "→ Test reports/2026-07 junit-test-results.xml", file: 0 },
    { tag: "→ file", cls: "tag--move", strong: "coverage-Q2-2026.html", desc: "→ Coverage/2026-Q2 coverage.html", file: 1 },
    { tag: "→ file", cls: "tag--move", strong: "build-log-2026-07-12.txt", desc: "→ Build logs/2026-07-12 build-log.txt", file: 2 },
    { tag: "→ file", cls: "tag--move", strong: "screenshot-flaky-login.png", desc: "→ Screenshots/screenshot-flaky-login.png", file: 3 },
    { tag: "→ file", cls: "tag--move", strong: "trace-run-8842.zip", desc: "→ Traces/trace-run-8842.zip", file: 4 },
    { tag: "✕ denied", cls: "tag--deny", strong: "0 disk writes", desc: "preview only — Organizer executes", deny: true },
  ];

  let state = "idle";

  function fileRow(f, i) {
    const li = el("li", null, `<span class="fname">${f.name}</span><span class="fmeta">${f.meta}</span>`);
    li.dataset.file = String(i);
    li.style.animationDelay = reduce.matches ? "0s" : `${i * 0.06}s`;
    return li;
  }

  function planRow(a) {
    return el(
      "li",
      a.deny ? "is-deny" : null,
      `<span class="tag ${a.cls}">${a.tag}</span> <span class="p-strong">${a.strong}</span> <span class="p-desc">${a.desc}</span>`,
    );
  }

  function reset() {
    filesEl.innerHTML = "";
    planEl.innerHTML = "";
    auditEl.innerHTML = "";
    auditBox.hidden = true;
    planHead.textContent = "Filing preview";
    hint.className = "demo__hint";
    hint.textContent = "Reads names only — Plan Filing never touches disk. Organizer adds approve, audit, and undo.";
    btn.textContent = "Preview filing";
    btn.disabled = false;
    state = "idle";
  }

  async function preview() {
    btn.disabled = true;
    hint.textContent = "Building a local filing preview from pasted names…";
    FILES.forEach((f, i) => filesEl.appendChild(fileRow(f, i)));
    await sequence(
      PLAN.map((a) => () => planEl.appendChild(planRow(a))),
      220,
    );
    planHead.textContent = "Filing preview — disk untouched";
    auditBox.hidden = false;
    auditEl.appendChild(
      el(
        "li",
        null,
        "✓ Preview ready — hand off to <b>Organizer</b> for approve → execute → audit → undo",
      ),
    );
    hint.textContent = "Same trust pipeline as the desktop app: Plan Filing proposes; Organizer acts only after you approve.";
    btn.textContent = "Reset preview";
    btn.disabled = false;
    state = "previewed";
  }

  btn.addEventListener("click", () => {
    if (state === "idle") preview();
    else if (state === "previewed") reset();
  });

  reset();
}

/* ============================================================
   Demo C — Record → Review → Replay → Audit → Undo
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

  const STEPS = [
    { icon: "🖱️", html: 'Click <b>“Export report”</b>' },
    { icon: "⌨️", html: 'Type <span class="redacted">redacted</span> into <b>Filename</b>' },
    { icon: "🔒", html: 'Password field — <span class="secure">not captured</span>' },
    { icon: "🖱️", html: 'Click <b>“Save”</b>' },
    { icon: "⏳", html: "Wait for download (2.1s)" },
  ];

  let state = "idle";

  function stepRow(s) {
    return el("li", null, `<span class="s-check">○</span> <span>${s.icon}</span> <span>${s.html}</span>`);
  }
  function auditRow(html) {
    return el("li", null, `✓ ${html}`);
  }

  function reset() {
    stepsEl.innerHTML = "";
    reviewEl.innerHTML = "";
    auditEl.innerHTML = "";
    auditBox.hidden = true;
    dot.classList.remove("is-live");
    title.textContent = "Ready to record";
    hint.className = "demo__hint";
    hint.textContent = "Typed text is redacted by default; secure fields are never captured.";
    btn.textContent = "Record a task";
    btn.disabled = false;
    state = "idle";
  }

  async function record() {
    btn.disabled = true;
    dot.classList.add("is-live");
    title.textContent = "Recording…";
    hint.textContent = "Capturing your actions — and redacting what's sensitive.";
    await sequence(
      STEPS.map((s) => () => stepsEl.appendChild(stepRow(s))),
      420,
    );
    dot.classList.remove("is-live");
    title.textContent = `${STEPS.length} steps captured`;
    hint.textContent = "Review the compressed steps before anything replays.";
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
    hint.textContent = "You approve every step. Deny-by-default until you say go.";
    btn.textContent = "Approve & replay";
    btn.disabled = false;
    state = "reviewed";
  }

  async function replay() {
    btn.disabled = true;
    hint.className = "demo__hint";
    hint.textContent = "Replaying your approved steps…";
    auditBox.hidden = false;
    const rows = $$("li", stepsEl);
    await sequence(
      STEPS.map((s, i) => () => {
        const check = $(".s-check", rows[i]);
        rows[i]?.classList.add("is-done");
        if (check) check.textContent = "●";
        auditEl.appendChild(auditRow(s.html.replace(/<[^>]+>/g, "").trim() || "step"));
      }),
      420,
    );
    hint.textContent = "Replay complete — every click's target traced, logged, and reversible.";
    btn.textContent = "↩︎ Undo replay";
    btn.disabled = false;
    state = "replayed";
  }

  async function undo() {
    btn.disabled = true;
    hint.textContent = "Undoing the replay's effects…";
    auditEl.appendChild(auditRow("Reverted: file export removed, prior state restored"));
    await sequence([() => {}], 500);
    hint.textContent = "Back to where you started.";
    btn.textContent = "Record again";
    btn.disabled = false;
    state = "undone";
  }

  btn.addEventListener("click", () => {
    if (state === "idle") record();
    else if (state === "recorded") review();
    else if (state === "reviewed") replay();
    else if (state === "replayed") undo();
    else if (state === "undone") reset();
  });

  reset();
}

/* ============================================================
   Demo D — Guard Desk → approve → POS Bridge
   ============================================================ */
function guardDeskDemo() {
  const rulesEl = $("#guard-rules");
  const verdictEl = $("#guard-verdict");
  const auditBox = $("#guard-audit");
  const auditEl = $("#guard-auditlist");
  const btn = $("#guard-action");
  const hint = $("#guard-hint");
  const planHead = $("#guard-planhead");
  if (!btn) return;

  const RULES = [
    { tag: "✓ pass", cls: "tag--new", strong: "Payee matches ID", desc: "JOHN DOE" },
    { tag: "✓ pass", cls: "tag--new", strong: "ID not expired", desc: "2029-12-15" },
    { tag: "✓ pass", cls: "tag--new", strong: "Check within 90 days", desc: "2026-07-09" },
    { tag: "✓ pass", cls: "tag--new", strong: "Signature reviewed", desc: "on device" },
    { tag: "✓ pass", cls: "tag--new", strong: "Under cashing limit", desc: "$1,450.00" },
  ];

  let state = "idle";

  function row(a) {
    return el(
      "li",
      null,
      `<span class="tag ${a.cls}">${a.tag}</span> <span class="p-strong">${a.strong}</span> <span class="p-desc">${a.desc || ""}</span>`,
    );
  }

  function reset() {
    rulesEl.innerHTML = "";
    verdictEl.innerHTML = "";
    auditEl.innerHTML = "";
    auditBox.hidden = true;
    planHead.textContent = "Verdict";
    hint.className = "demo__hint";
    hint.textContent = "Local compliance check — nothing leaves your browser.";
    btn.textContent = "Scan documents";
    btn.disabled = false;
    state = "idle";
  }

  async function scan() {
    btn.disabled = true;
    hint.textContent = "Scanning check + ID locally…";
    await sequence(
      RULES.map((a) => () => rulesEl.appendChild(row(a))),
      220,
    );
    planHead.textContent = "Verdict — APPROVED";
    verdictEl.appendChild(
      row({ tag: "approve", cls: "tag--move", strong: "Safe to cash", desc: "awaiting your approval" }),
    );
    hint.textContent = "Ghost recommends. You still approve before POS Bridge types.";
    btn.textContent = "Approve plan";
    btn.disabled = false;
    state = "scanned";
  }

  async function approve() {
    btn.disabled = true;
    hint.textContent = "Approved — POS Bridge unlocked.";
    verdictEl.appendChild(
      row({ tag: "✓ you", cls: "tag--new", strong: "Human approved", desc: "plan locked" }),
    );
    btn.textContent = "Auto-fill POS";
    btn.disabled = false;
    state = "approved";
  }

  async function autofill() {
    btn.disabled = true;
    auditBox.hidden = false;
    hint.textContent = "Typing into POS Bridge…";
    const fields = [
      "Payee Full Name → JOHN DOE",
      "ID Number → DL-98234812",
      "Check Amount → 1,450.00",
      "Routing → 121000248",
      "Account → 987654321",
      "Status → OK-88294",
    ];
    await sequence(
      fields.map((f) => () => {
        const li = el("li", null, `✓ ${f}`);
        auditEl.appendChild(li);
      }),
      280,
    );
    hint.textContent = "Done — every field typed after your approval.";
    btn.textContent = "Run again";
    btn.disabled = false;
    state = "done";
  }

  btn.addEventListener("click", () => {
    if (state === "idle") scan();
    else if (state === "scanned") approve();
    else if (state === "approved") autofill();
    else if (state === "done") reset();
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
  organizerDemo();
  planFilingDemo();
  replayDemo();
  guardDeskDemo();
});
