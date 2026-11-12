// Første gang: onboarding vises bare når appen kjøres første gang (ingen mappe valgt, onboarding ikke fullført).
// Avbrutt skann (app lukket) og senere starter går rett til Hjem. Ingen I/O her utenom bridge.
import type { CoreBridge } from "../bridge/core-bridge";
import type { Route } from "../router";

/** true = send brukeren til onboarding. Bare fra Hjem (dypelenker til andre skjermer respekteres). Feil gir aldri onboarding. */
export async function shouldShowOnboarding(b: CoreBridge, route: Route): Promise<boolean> {
  if (route.name !== "home") return false;
  try {
    return await b.getFirstRun();
  } catch {
    return false;
  }
}

/** Markerer onboarding som gjennomført. Feil svelges: det verste er at velkomst vises én gang til. */
export async function markOnboarded(b: CoreBridge): Promise<void> {
  try {
    await b.completeOnboarding();
  } catch {
    /* ikke kritisk */
  }
}
