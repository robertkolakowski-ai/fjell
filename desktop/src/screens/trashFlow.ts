// Papirkurv: flyt mot CoreBridge (brukes av Trash.svelte, testes mot MockBridge).
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";
import { ackFor, initialTrash, reduceTrash, type TrashEvent, type TrashState } from "./trashState";

export class TrashSession {
  state: TrashState = initialTrash;
  private gone = false;

  constructor(private readonly b: CoreBridge, private readonly onChange: (s: TrashState) => void = () => undefined) {}

  send(e: TrashEvent): void {
    if (this.gone) return;
    this.state = reduceTrash(this.state, e);
    this.onChange(this.state);
  }
  dispose(): void { this.gone = true; }

  async load(): Promise<void> {
    try {
      const [state, items] = await Promise.all([this.b.getState(), this.b.listTrashed()]);
      this.send({ type: "loaded", state, items });
    } catch (e) {
      this.send({ type: "failed", error: e as BridgeError });
    }
  }

  /** Fortsett (Papirkurv) eller Slett permanent. Utfører bare når reduceren har gått til «purging». */
  async confirm(): Promise<void> {
    this.send(this.state.phase === "confirmPermanent" ? { type: "confirmPermanent" } : { type: "continue" });
    if (this.state.phase !== "purging") return;
    const mode = this.state.mode;
    const ack = ackFor(this.state);
    try {
      const result = await this.b.emptyTrash(ack);
      this.send({ type: "purged", mode, result });
      await this.load();
    } catch (e) {
      this.send({ type: "purgeFailed", error: e as BridgeError });
    }
  }

  /** «Gjenopprett» (valgte) eller «Gjenopprett alt». Umiddelbart, uten bekreftelse. Feil: ingenting er endret. */
  async restore(all: boolean): Promise<void> {
    const ids = [...this.state.selected];
    if (!all && ids.length === 0) return;
    // Bare én gjenoppretting om gangen (dobbelttrykk gir ett kall).
    if (this.state.restoring || this.state.phase !== "list") return;
    this.send({ type: "restoring" });
    try {
      const r = all ? await this.b.restoreAll() : await this.b.restore(ids);
      this.send({ type: "restoredOk", count: r.restored });
      await this.load();
    } catch {
      this.send({ type: "restoreFailed" });
    }
  }
}
