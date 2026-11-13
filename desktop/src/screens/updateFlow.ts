// Oppdateringer (M9): flyt mot CoreBridge. Brukes av UpdateSection.svelte (Innstillinger) og App.svelte (stille sjekk ved oppstart).
import { writable } from "svelte/store";
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError, UpdateCheck } from "../bridge/types";
import { initialUpdate, isScanning, reduceUpdate, type UpdateEvent, type UpdateView } from "./updateState";

/** Funnet ny versjon (fra stille sjekk ved oppstart eller fra knappen). Hjem viser banner når den er satt. Aldri installasjon. */
export const pendingUpdate = writable<{ version: string } | null>(null);

export class UpdateSession {
  state: UpdateView = initialUpdate;
  private gone = false;

  constructor(private readonly b: CoreBridge, private readonly onChange: (s: UpdateView) => void = () => undefined) {}

  send(e: UpdateEvent): void {
    if (this.gone) return;
    this.state = reduceUpdate(this.state, e);
    this.onChange(this.state);
  }
  dispose(): void { this.gone = true; }

  /** Leser versjon, auto-valg og skannstatus. Ingen nettverkskall (bridge sender ingenting her). */
  async load(): Promise<void> {
    await Promise.all([
      this.b.getAppVersion().then((version) => this.send({ type: "version", version }), () => this.send({ type: "version", version: "dev" })),
      this.b.getAutoUpdate().then((on) => this.send({ type: "auto", on }), () => undefined),
      this.refreshScan(),
    ]);
  }

  async refreshScan(): Promise<void> {
    try {
      this.send({ type: "scan", scanning: isScanning(await this.b.getState()) });
    } catch { /* uendret: install sjekker på nytt rett før den starter */ }
  }

  /** «Se etter oppdatering». Kalles bare fra knappetrykk. */
  async check(): Promise<void> {
    if (this.state.status === "checking" || this.state.status === "installing" || this.state.status === "restarting") return;
    this.send({ type: "checking" });
    try {
      const result = await this.b.checkForUpdate();
      pendingUpdate.set(result.available && result.version ? { version: result.version } : null);
      this.send({ type: "checked", result });
    } catch (e) {
      this.send({ type: "checkFailed", error: e as BridgeError });
    }
  }

  /** «Installer og start på nytt». Leser skannstatus på nytt først: aldri installasjon mens skanning pågår. */
  async install(): Promise<void> {
    const s = this.state;
    if ((s.status !== "available" && s.status !== "installFailed") || !s.found) return;
    await this.refreshScan();
    if (this.state.scanning) return;
    this.send({ type: "installing" });
    if (this.state.status !== "installing") return;
    try {
      await this.b.installUpdate((done, total) => this.send({ type: "progress", done, total }));
      this.send({ type: "restarting" });
    } catch (e) {
      this.send({ type: "installFailed", error: e as BridgeError });
    }
  }

  async setAuto(on: boolean): Promise<void> {
    if (this.state.autoSaving || this.state.auto === on) return;
    this.send({ type: "autoSaving" });
    try {
      this.send({ type: "auto", on: await this.b.setAutoUpdate(on) });
    } catch {
      this.send({ type: "autoFailed" });
    }
  }
}

/**
 * Stille sjekk ved oppstart, bare når brukeren har slått på «Sjekk automatisk ved oppstart» (standard av).
 * Feil er stille (ingen banner, ingen melding). Installerer aldri. Returnerer funnet oppdatering eller null.
 */
export async function runStartupCheck(b: CoreBridge): Promise<UpdateCheck | null> {
  try {
    if (!(await b.getAutoUpdate())) return null;
    const r = await b.checkForUpdate();
    pendingUpdate.set(r.available && r.version ? { version: r.version } : null);
    return r;
  } catch {
    return null;
  }
}
