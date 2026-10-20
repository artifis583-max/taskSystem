export const BOUNCE = "cubic-bezier(0.34, 1.56, 0.64, 1)";
export const EASE_OUT = "cubic-bezier(0.2, 0.8, 0.2, 1)";

/** Timings from the plan, §3. */
export const T = {
  tick: 100,
  swipe: 300,
  hold: 350,
  move: 520, // move-to-bottom + landing bounce
  shift: 350, // other cards sliding into place
};

export const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

export function prefersReducedMotion(): boolean {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}
