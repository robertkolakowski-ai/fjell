// Skymappe: hjelpere for skjermene. Kjernen (skallet) er sannheten; UI sperrer papirkurv synlig med forklaring og skjuler aldri noe.
// Deteksjonen går etter mappenavn og er et sikkerhetsnett. Leverandørkoden (icloud, onedrive, dropbox) brukes som tekstvalg;
// en ukjent kode gir nøytral tekst («en skytjeneste»).
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";

/** Leverandørkode for gjeldende bibliotek, eller null (ikke skysynket, eller tilstanden kan ikke leses). */
export async function loadCloud(b: CoreBridge): Promise<string | null> {
  try {
    return (await b.getState()).cloudSync || null;
  } catch {
    return null;
  }
}

/** Skallet nektet papirkurv fordi mappen er skysynket (kan skje selv om UI ikke visste det ennå). Gir leverandørkoden. */
export const cloudFromError = (e: BridgeError | null | undefined): string | null =>
  e?.code === "cloud_synced_readonly" ? e.detail || "cloud" : null;
