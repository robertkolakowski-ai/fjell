// Hash-ruter uten avhengigheter. Ren parse/format (testbar i node) + en liten store rundt `location.hash`.
// Hash (ikke History API) fungerer likt i nettleser/PWA-dev og Tauri-WebView2 uten server-fallback.
import { KIND_CODES, type KindCode } from "./bridge/types";

export type Route =
  | { name: "onboarding" }
  | { name: "home" }
  | { name: "review"; kind: KindCode | null }
  | { name: "group"; id: number }
  | { name: "cards"; kind: KindCode | null }
  | { name: "trash" }
  | { name: "settings" };

export type RouteName = Route["name"];
/** Toppnivå-valg i navigasjonen. Group/Cards/Onboarding er undersider (Gruppe hører til Gjennomgang). */
export const NAV_ROUTES = ["home", "review", "trash", "settings"] as const satisfies readonly RouteName[];
export type NavRoute = (typeof NAV_ROUTES)[number];

export const HOME: Route = { name: "home" };

const isKind = (s: string | undefined): s is KindCode => !!s && (KIND_CODES as readonly string[]).includes(s);

/** `#/review/blurry` -> Route. Ukjent eller ugyldig hash gir Hjem (aldri blank skjerm). */
export function parseHash(hash: string): Route {
  const [name, arg] = hash.replace(/^#\/?/, "").split("?")[0].split("/").filter(Boolean);
  switch (name) {
    case "onboarding": return { name: "onboarding" };
    case "review": return { name: "review", kind: isKind(arg) ? arg : null };
    case "cards": return { name: "cards", kind: isKind(arg) ? arg : null };
    case "group": {
      const id = Number(arg);
      return Number.isInteger(id) && id > 0 ? { name: "group", id } : HOME;
    }
    case "trash": return { name: "trash" };
    case "settings": return { name: "settings" };
    default: return HOME;
  }
}

export function formatHash(r: Route): string {
  switch (r.name) {
    case "review":
    case "cards": return r.kind ? `#/${r.name}/${r.kind}` : `#/${r.name}`;
    case "group": return `#/group/${r.id}`;
    default: return `#/${r.name}`;
  }
}

/** Hvilket navigasjonsvalg som er aktivt for en rute. */
export function navFor(r: Route): NavRoute | null {
  switch (r.name) {
    case "home": return "home";
    case "review":
    case "group":
    case "cards": return "review";
    case "trash": return "trash";
    case "settings": return "settings";
    case "onboarding": return null;
  }
}

/** Hash for et toppnivåvalg. */
export const navHash = (n: NavRoute): string => `#/${n}`;

export function navigate(r: Route): void {
  if (typeof location !== "undefined") location.hash = formatHash(r);
}

/** Svelte-kompatibel store over gjeldende rute. Lytter bare mens noen abonnerer. */
export const route = {
  subscribe(fn: (r: Route) => void): () => void {
    const read = () => fn(parseHash(typeof location === "undefined" ? "" : location.hash));
    read();
    if (typeof window === "undefined") return () => {};
    window.addEventListener("hashchange", read);
    return () => window.removeEventListener("hashchange", read);
  },
};
