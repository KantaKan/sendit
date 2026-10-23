import { quintOut } from 'svelte/easing';

/** The OS asked for less motion: keep fades and color, drop travel. */
export const reduce = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

/** Same curve as --ease in app.css. */
export const ease = quintOut;

/** Travel distance for a transition, or 0 under reduced motion. */
export const move = (px: number) => (reduce ? 0 : px);

/** Leaving list item: fades and drifts aside while the rows below glide up (animate:flip). */
export function leave(_node: HTMLElement, { duration = 200 } = {}) {
  return { duration, easing: ease, css: (t: number) => `opacity:${t};transform:translateX(${move(-12) * (1 - t)}px)` };
}
