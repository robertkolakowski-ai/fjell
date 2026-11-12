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
      this.send({ type: "loaded", state: await this.b.getState() });
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
}
