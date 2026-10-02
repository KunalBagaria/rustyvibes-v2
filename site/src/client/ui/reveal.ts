/** Fade-up reveals and number count-ups, both only once and never while idle. */

const reducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

function countUp(el: HTMLElement): void {
  const to = Number(el.dataset.countTo);
  const decimals = Number(el.dataset.decimals ?? 0);
  if (!Number.isFinite(to) || to === 0 || reducedMotion()) return;
  const duration = 1100;
  const start = performance.now();
  const frame = (now: number) => {
    const t = Math.min(1, (now - start) / duration);
    const eased = 1 - (1 - t) ** 3;
    el.textContent = (to * eased).toFixed(decimals);
    if (t < 1) requestAnimationFrame(frame);
  };
  el.textContent = (0).toFixed(decimals);
  requestAnimationFrame(frame);
}

export function initReveal(): void {
  const items = [...document.querySelectorAll<HTMLElement>("[data-reveal]")];
  // Anything already on screen shows at once, so enabling reveals never makes it flash.
  for (const el of items) {
    if (el.getBoundingClientRect().top < window.innerHeight) el.classList.add("is-visible");
  }
  document.documentElement.classList.add("reveal");
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        entry.target.classList.add("is-visible");
        observer.unobserve(entry.target);
      }
    },
    { rootMargin: "0px 0px -8% 0px", threshold: 0.1 },
  );
  for (const el of items) if (!el.classList.contains("is-visible")) observer.observe(el);

  const counters = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        countUp(entry.target as HTMLElement);
        counters.unobserve(entry.target);
      }
    },
    { threshold: 0.6 },
  );
  for (const el of document.querySelectorAll<HTMLElement>("[data-count-to]")) counters.observe(el);
}
