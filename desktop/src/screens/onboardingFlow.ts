// Effekter for onboarding: rask skann, første funn og «flytt til papirkurv». Alt går via CoreBridge.
import type { CoreBridge } from "../bridge/core-bridge";
import type { BridgeError } from "../bridge/types";

export interface ExactFound {
  count: number;
  bytes: number;
  ids: number[];
}

export type QuickScanResult =
  | { kind: "ready"; found: ExactFound }
  | { kind: "denied" }
  | { kind: "failed" };

export interface QuickScanOptions {
  onFound?: (count: number) => void;
  /** Kalles én gang hvis skann ikke er ferdig etter `partialAfterMs` (45 s). Skann og polling fortsetter. */
  onSlow?: () => void;
  intervalMs?: number;
  partialAfterMs?: number;
  sleep?: (ms: number) => Promise<void>;
  now?: () => number;
  /** true = brukeren forlot skjermen; polling stopper. */
  cancelled?: () => boolean;
}

const DENIED = new Set(["no_access", "NoAccess"]);
const defaultSleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/** Alle eksakte duplikat-kort (id-er) via markørpaginering. */
export async function allExactIds(bridge: CoreBridge): Promise<number[]> {
  const ids: number[] = [];
  let cursor: number | null = null;
  do {
    const page = await bridge.listCards("exact_dup", 200, cursor);
    ids.push(...page.cards.map((c) => c.id));
    cursor = page.next;
  } while (cursor !== null);
  return ids;
}

async function exactSummary(bridge: CoreBridge): Promise<{ count: number; bytes: number; ready: boolean }> {
  const st = await bridge.getState();
  const k = st.perKind.find((x) => x.kind === "exact_dup");
  const ready = st.scanPhase === "exact_ready" || st.scanPhase === "analyzing" || st.scanPhase === "done";
  return { count: k?.count ?? 0, bytes: k?.bytes ?? 0, ready };
}

/** Starter rask skann (full = false) og venter til eksakte duplikater er klare. */
export async function quickScan(bridge: CoreBridge, o: QuickScanOptions = {}): Promise<QuickScanResult> {
  const { intervalMs = 500, partialAfterMs = 45_000, sleep = defaultSleep, now = Date.now } = o;
  const verdict = (e: unknown): QuickScanResult => (DENIED.has((e as BridgeError | undefined)?.code ?? "") ? { kind: "denied" } : { kind: "failed" });
  let startError: unknown = null;
  let failed = false;
  // Ikke await: skann kan rapportere fremdrift mens kallet pågår.
  void bridge.startScan(false).catch((e: unknown) => { failed = true; startError = e; });
  const t0 = now();
  let slowSent = false;
  try {
    for (;;) {
      if (o.cancelled?.()) return { kind: "failed" };
      if (failed) return verdict(startError);
      const s = await exactSummary(bridge);
      if (failed) return verdict(startError);
      o.onFound?.(s.count);
      if (s.ready) {
        const ids = s.count > 0 ? await allExactIds(bridge) : [];
        return { kind: "ready", found: { count: s.count, bytes: s.bytes, ids } };
      }
      if (!slowSent && now() - t0 >= partialAfterMs) {
        slowSent = true;
        o.onSlow?.();
      }
      await sleep(intervalMs);
    }
  } catch (e) {
    return verdict(e);
  }
}

/** Ett trykk: alle funn til papirkurven (én batch = ett angre). MassAck er sjekket av kalleren. */
export const trashFound = (bridge: CoreBridge, ids: number[]): Promise<number> => bridge.decide(ids, "trash");
