// Liten debounce brukt av LiveRegion (300 ms demping, PLAN-UX-I18N 6.2). Ingen avhengigheter.
export interface Debounced<A extends unknown[]> {
  (...args: A): void;
  cancel(): void;
}

export function debounce<A extends unknown[]>(fn: (...args: A) => void, ms: number | (() => number)): Debounced<A> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const d = ((...args: A) => {
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      fn(...args);
    }, typeof ms === "function" ? ms() : ms);
  }) as Debounced<A>;
  d.cancel = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  return d;
}
