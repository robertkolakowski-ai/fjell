// Bridge via Svelte-kontekst: App setter den én gang, skjermer henter den. Props kan overstyre (tester).
import { getContext, setContext } from "svelte";
import type { CoreBridge } from "./bridge/core-bridge";

const KEY = Symbol("fjell.bridge");

export function provideBridge(b: CoreBridge): void {
  setContext(KEY, b);
}

/** Bridge fra prop hvis gitt, ellers fra kontekst. Kaster tydelig hvis ingen finnes (programmeringsfeil). */
export function useBridge(override?: CoreBridge): CoreBridge {
  const b = override ?? getContext<CoreBridge | undefined>(KEY);
  if (!b) throw new Error("useBridge: ingen bridge i kontekst (mangler provideBridge i App?)");
  return b;
}
