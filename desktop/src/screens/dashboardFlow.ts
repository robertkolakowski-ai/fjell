// Oversikt: henting mot CoreBridge (brukes av DashboardPanel.svelte, testes mot MockBridge). Bare lesing.
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";
import { initialDashboard, reduceDashboard, type DashboardEvent, type DashboardView } from "./dashboardState";

export class DashboardSession {
  state: DashboardView = initialDashboard;
  private gone = false;

  constructor(private readonly b: CoreBridge, private readonly onChange: (s: DashboardView) => void = () => undefined) {}

  send(e: DashboardEvent): void {
    if (this.gone) return;
    this.state = reduceDashboard(this.state, e);
    this.onChange(this.state);
  }
  dispose(): void { this.gone = true; }

  async load(): Promise<void> {
    try {
      this.send({ type: "loaded", data: await this.b.getDashboard() });
    } catch (e) {
      this.send({ type: "failed", error: e as BridgeError });
    }
  }
  async retry(): Promise<void> {
    this.send({ type: "retry" });
    await this.load();
  }
}
