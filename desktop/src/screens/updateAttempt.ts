// Oppdateringer: merke for «installasjon startet». På Windows avslutter appen seg selv når installeren starter, så en mislykket
// installasjon (f.eks. NSIS «Kunne ikke avslutte Fjell») kan bare oppdages ved neste start: merket finnes fortsatt, og
// versjonen er eldre enn den vi prøvde å installere. Da viser UpdateSection reserveveien (manuell nedlasting).
// Bare versjonsnummeret lagres (localStorage i appens WebView-profil). Ingen nettverk, ingen stier.

export interface AttemptStore {
  /** Versjonen vi sist prøvde å installere, eller null. */
  get(): string | null;
  set(version: string): void;
  clear(): void;
}

const KEY = "fjell.update.attempt";

/** localStorage-basert. Blokkert eller manglende lagring gir bare «ingen merke»: reserveveien ved kastet feil virker uansett. */
export const localAttempts: AttemptStore = {
  get: () => { try { return globalThis.localStorage?.getItem(KEY) ?? null; } catch { return null; } },
  set: (version) => { try { globalThis.localStorage?.setItem(KEY, version); } catch { /* ignorer */ } },
  clear: () => { try { globalThis.localStorage?.removeItem(KEY); } catch { /* ignorer */ } },
};

/** Minne-basert (tester). */
export function memoryAttempts(initial: string | null = null): AttemptStore {
  let v = initial;
  return { get: () => v, set: (x) => { v = x; }, clear: () => { v = null; } };
}

const parse = (v: string): number[] | null => (/^\d+(\.\d+){0,3}$/.test(v) ? v.split(".").map(Number) : null);

/** Er `current` en eldre versjon enn `target`? Bare rene tallversjoner (0.1.42); alt annet («dev») gir false. */
export function olderThan(current: string, target: string): boolean {
  const a = parse(current), b = parse(target);
  if (!a || !b) return false;
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const d = (a[i] ?? 0) - (b[i] ?? 0);
    if (d !== 0) return d < 0;
  }
  return false;
}
