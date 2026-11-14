// Innstillinger: flyt mot CoreBridge (brukes av Settings.svelte, testes mot MockBridge).
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";
import { initialSettings, reduceSettings, type SettingsEvent, type SettingsState } from "./settingsState";

export class SettingsSession {
  state: SettingsState = initialSettings;
  private gone = false;

  constructor(private readonly b: CoreBridge, private readonly onChange: (s: SettingsState) => void = () => undefined) {}

  send(e: SettingsEvent): void {
    if (this.gone) return;
    this.state = reduceSettings(this.state, e);
    this.onChange(this.state);
  }
  dispose(): void { this.gone = true; }

  async load(): Promise<void> {
    try {
      this.send({ type: "loaded", settings: await this.b.getSettings() });
    } catch (e) {
      this.send({ type: "failed", error: e as BridgeError });
    }
  }

  /** Endrer papirkurvfristen. Samme verdi eller pågående lagring: ingen kall. Feil: verdien er uendret. */
  async setTrashDays(days: number): Promise<void> {
    const cur = this.state.settings;
    if (!cur || this.state.saving || cur.trashDays === days) return;
    this.send({ type: "saving" });
    try {
      this.send({ type: "saved", settings: await this.b.setTrashDays(days) });
    } catch {
      this.send({ type: "saveFailed" });
    }
  }
}
