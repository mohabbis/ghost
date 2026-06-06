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

  document.querySelectorAll(".section, .hero-card, .feature-card").forEach((element) => {
    element.classList.add("reveal");
    observer.observe(element);
  });
}

window.addEventListener("DOMContentLoaded", revealOnScroll);
