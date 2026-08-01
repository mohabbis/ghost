/* ============================================================
   Ghost marketing site — interactions.

   Progressive enhancement only: the page is fully readable with
   this file blocked. Nothing here fetches, tracks, or renders
   product claims — those live in the markup so they stay
   reviewable.
   ============================================================ */

const reduce = window.matchMedia("(prefers-reduced-motion: reduce)");

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

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
  setupScrollSpy();
});
