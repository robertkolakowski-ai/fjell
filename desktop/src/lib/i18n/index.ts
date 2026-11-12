// i18n-runtime (UX-0). Kataloger er generert av tools/gen-i18n til ./strings/ (innsjekket, ikke rediger).
//
//   import { t, tr, setLanguage } from "$lib/i18n";   // i Svelte: {$tr("home.title")} (reaktiv) eller t(...) (engangs)
//
// Språkvalg: «system» (standard) følger navigator.languages (WebView2 = Windows-visningsspråk), ellers nb/en.
// «pseudo» finnes for UI-tester og er ikke et brukervalg. Valget lagres i localStorage («fjell.language»)
// inntil Innstillinger får egen lagring. Manglende oversettelse faller nøkkelvis tilbake til nb (nn kommer etter MVP).
import { formatMessage, type Params } from "./icu";
import { formatBytes, formatDate, formatNumber, formatPercent } from "./format";
import type { MessageKey, MessageParams } from "./strings/keys";
import { nb } from "./strings/nb";
import { en } from "./strings/en";
import { pseudo } from "./strings/pseudo";

export type { MessageKey, MessageParams } from "./strings/keys";
export { SHORT_KEYS } from "./strings/keys";

export type LocaleId = "nb" | "en" | "pseudo";
export type LanguagePref = "system" | LocaleId;

type Catalog = Readonly<Partial<Record<MessageKey, string>>>;
const CATALOGS: Readonly<Record<LocaleId, Catalog>> = { nb, en, pseudo };
/** Intl-tag per språk. Pseudo bruker nb sine regler (det er nb-tekst som er forlenget). */
const INTL: Readonly<Record<LocaleId, string>> = { nb: "nb-NO", en: "en", pseudo: "nb-NO" };
const STORAGE_KEY = "fjell.language";
export const FALLBACK_LOCALE: LocaleId = "en";

type Args<K extends MessageKey> = keyof MessageParams[K] extends never ? [params?: undefined] : [params: MessageParams[K]];
export type TFunction = <K extends MessageKey>(key: K, ...args: Args<K>) => string;

/** Systemspråk (BCP 47-liste, mest foretrukket først) -> støttet språk. nb/nn/no -> nb, en -> en, ellers en. */
export function resolveSystemLocale(languages: readonly string[]): LocaleId {
  for (const raw of languages) {
    const base = raw.toLowerCase().split("-")[0];
    if (base === "nb" || base === "nn" || base === "no") return "nb";
    if (base === "en") return "en";
  }
  return FALLBACK_LOCALE;
}

function readStorage(): LanguagePref {
  try {
    const v = globalThis.localStorage?.getItem(STORAGE_KEY);
    if (v === "nb" || v === "en" || v === "pseudo" || v === "system") return v;
  } catch {
    /* lagring kan være blokkert; standard gjelder */
  }
  return "system";
}

function systemLanguages(): readonly string[] {
  return typeof navigator === "undefined" ? [] : (navigator.languages?.length ? navigator.languages : [navigator.language ?? ""]);
}

function lookup(locale: LocaleId, key: MessageKey): string {
  const msg = CATALOGS[locale][key] ?? nb[key];
  if (msg === undefined) {
    warnOnce(`mangler nøkkel «${key}»`);
    return key;
  }
  return msg;
}

const warned = new Set<string>();
function warnOnce(m: string): void {
  if (warned.has(m)) return;
  warned.add(m);
  console.warn(`i18n: ${m}`);
}

function makeT(locale: LocaleId): TFunction {
  const ctx = { intl: INTL[locale] };
  return ((key: MessageKey, params?: Params) =>
    formatMessage(lookup(locale, key), params ?? {}, ctx, (n) => warnOnce(`${key}: manglende parameter «${n}»`))) as TFunction;
}

let pref: LanguagePref = readStorage();
let locale: LocaleId = pref === "system" ? resolveSystemLocale(systemLanguages()) : pref;
let current: TFunction = makeT(locale);
const listeners = new Set<(t: TFunction) => void>();

function applyDocumentLang(): void {
  if (typeof document !== "undefined") document.documentElement.lang = locale === "pseudo" ? "nb" : locale;
}
applyDocumentLang();

/** Oversett. `t("home.ring.caption", { size })`; typer for parametre følger nb-malen. */
export const t: TFunction = (key, ...args) => current(key, ...args);

/** Svelte-kompatibel store: `$tr("nøkkel", {…})` oppdateres når språket byttes. */
export const tr = {
  subscribe(fn: (t: TFunction) => void): () => void {
    fn(current);
    listeners.add(fn);
    return () => void listeners.delete(fn);
  },
};

export function getLanguage(): LanguagePref {
  return pref;
}
export function getLocale(): LocaleId {
  return locale;
}
/** Intl-tag for gjeldende språk, til formatering utenfor t(). */
export function getIntlLocale(): string {
  return INTL[locale];
}

/** Bytt språk (fra Innstillinger `setting.language`). `languages` er bare til test. */
export function setLanguage(next: LanguagePref, languages: readonly string[] = systemLanguages()): void {
  pref = next;
  locale = next === "system" ? resolveSystemLocale(languages) : next;
  current = makeT(locale);
  try {
    globalThis.localStorage?.setItem(STORAGE_KEY, next);
  } catch {
    /* ignorer */
  }
  applyDocumentLang();
  for (const fn of listeners) fn(current);
}

/** Formatterere bundet til gjeldende språk. */
export const fmt = {
  bytes: (n: number) => formatBytes(n, INTL[locale]),
  percent: (fraction: number) => formatPercent(fraction, INTL[locale]),
  number: (n: number) => formatNumber(n, INTL[locale]),
  date: (d: Date | number) => formatDate(d, INTL[locale]),
};

/** Kjernekode (snake_case) -> begrunnelsesmerke. Ukjent kode gir nøytral «Forslag» og logges. */
export function reasonLabel(code: string, params: { pct?: string } = {}): string {
  const key = `reason.${code}` as MessageKey;
  if (!(key in nb)) {
    warnOnce(`ukjent ReasonCode «${code}»`);
    return current("ui.card.suggestion");
  }
  return (current as (k: MessageKey, p?: Params) => string)(key, params as Params);
}
