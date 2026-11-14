// Effekter for Hjem: skannkontroller og «Legg til mappe». Alt går via CoreBridge (testes mot MockBridge).
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";

export type ScanNote = "cancelled" | "error" | null;

/** Resultat av en skannkontroll: melding til brukeren (aldri tekst fra kjernen). */
export async function controlScan(b: CoreBridge, action: "pause" | "resume" | "cancel" | "start"): Promise<ScanNote> {
  try {
    if (action === "pause") await b.pauseScan();
    else if (action === "resume") await b.resumeScan();
    else if (action === "cancel") await b.cancelScan();
    else await b.startScan(true);
    return action === "cancel" ? "cancelled" : null;
  } catch {
    return "error";
  }
}

export type AddFolderResult = { ok: true; path: string } | { ok: false; error: BridgeError };

/**
 * Velger mappe og starter skann (`scan = false` i onboarding, som starter rask skann selv). Tom sti avvises uten kall. Mappe som ikke finnes gir `ok: false`, og
 * ingenting endres (forrige mappe beholdes). Bilder røres ikke: kjernen leser bare.
 */
export async function addFolder(b: CoreBridge, raw: string, scan = true): Promise<AddFolderResult> {
  const path = raw.trim();
  if (path === "") return { ok: false, error: { code: "not_a_folder", detail: "" } };
  try {
    await b.setLibrary(path);
  } catch (e) {
    return { ok: false, error: e as BridgeError };
  }
  if (scan) {
    try {
      await b.startScan(true);
    } catch (e) {
      return { ok: false, error: e as BridgeError };
    }
  }
  return { ok: true, path };
}

export type BrowseResult = { kind: "picked"; path: string } | { kind: "cancelled" } | { kind: "failed" };

/**
 * «Bla …»: åpner systemets mappevalg. Endrer ikke biblioteket (brukeren bekrefter med «Legg til»).
 * Avbrutt gir `cancelled` (feltet urørt); feil eller manglende mappevalg gir `failed` (aldri stille svikt).
 */
export async function browseFolder(b: CoreBridge, title?: string): Promise<BrowseResult> {
  if (!b.canPickFolder) return { kind: "failed" };
  try {
    const p = await b.pickFolder(title);
    return p === null || p.trim() === "" ? { kind: "cancelled" } : { kind: "picked", path: p };
  } catch {
    return { kind: "failed" };
  }
}
