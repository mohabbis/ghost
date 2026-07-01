const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

function revealOnScroll() {
  if (prefersReducedMotion.matches || !("IntersectionObserver" in window)) return;

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        entry.target.classList.add("is-visible");
        observer.unobserve(entry.target);
      });
    },
    { threshold: 0.16 },
  );

  document.querySelectorAll(".section, .ship-card, .step-card, .console").forEach((element) => {
    element.classList.add("reveal");
    observer.observe(element);
  });
}

function rotateShipStatus() {
  const status = document.getElementById("ship-status");
  if (!status || prefersReducedMotion.matches) return;

  const messages = [
    "Master restored to green after #80.",
    "Replay History is now visible in trusted core.",
    "macOS event-tap callbacks are panic-contained.",
    "v1.0.13 is queued for signed release prep.",
  ];

  let index = 0;
  setInterval(() => {
    index = (index + 1) % messages.length;
    status.textContent = messages[index];
  }, 3200);
}

window.addEventListener("DOMContentLoaded", () => {
  revealOnScroll();
  rotateShipStatus();
});
