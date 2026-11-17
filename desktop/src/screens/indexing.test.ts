// Indekseringsflyt (docs/v1/INDEKSERING.md): tilstandskoder til tekst, rolig forklaring, aria-live bare ved endring, og at
// knappene alltid har en gyldig handling (også «Fortsett» når PC-en går på batteri).
import { describe, expect, it, vi } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import type { IndexingStatus } from "../bridge/types-robust";
import { announceKey, hintKey, IDLE_STATUS, IndexingSession, isHalted, remaining, stateKey } from "./indexingFlow";

const mock = (o: ConstructorParameters<typeof MockBridge>[0] = {}) => new MockBridge({ now: () => Date.UTC(2026, 10, 15), ...o });
const st = (patch: Partial<IndexingStatus>): IndexingStatus => ({ ...IDLE_STATUS, ...patch });

describe("tilstandskoder", () => {
  it("hver tilstand har en tekst, og ukjent kode viser pause (aldri blank)", () => {
    expect(stateKey("running")).toBe("indexing.state.running");
    expect(stateKey("paused")).toBe("indexing.state.paused");
    expect(stateKey("paused_on_battery")).toBe("indexing.state.paused_on_battery");
    expect(stateKey("disk_offline")).toBe("indexing.state.disk_offline");
    expect(stateKey("noe_nytt")).toBe("indexing.state.paused");
  });

  it("batteri og frakoblet disk har en rolig forklaring; pause og kjører har ingen", () => {
    expect(hintKey("paused_on_battery")).toBe("indexing.hint.battery");
    expect(hintKey("disk_offline")).toBe("indexing.hint.disk_offline");
    expect(hintKey("paused")).toBeNull();
    expect(hintKey("running")).toBeNull();
  });

  it("isHalted: alt unntatt ledig og kjører", () => {
    expect(["idle", "running", "paused", "paused_on_battery", "disk_offline"].map(isHalted)).toEqual([false, false, true, true, true]);
  });

  it("remaining er aldri negativt", () => {
    expect(remaining(st({ done: 40, total: 100 }))).toBe(60);
    expect(remaining(st({ done: 120, total: 100 }))).toBe(0);
  });
});

describe("announceKey (aria-live)", () => {
  it("kunngjør bare endringer", () => {
    expect(announceKey("running", "running")).toBeNull();
    expect(announceKey("running", "paused")).toBe("indexing.state.paused");
    expect(announceKey("paused", "running")).toBe("indexing.state.running");
    expect(announceKey("running", "paused_on_battery")).toBe("indexing.state.paused_on_battery");
    expect(announceKey("running", "disk_offline")).toBe("indexing.state.disk_offline");
  });

  it("ferdig kunngjøres når noe pågikk, ikke ved første måling av ledig", () => {
    expect(announceKey("running", "idle")).toBe("indexing.live.done");
    expect(announceKey("paused", "idle")).toBe("indexing.live.done");
    expect(announceKey(null, "idle")).toBeNull();
    expect(announceKey("idle", "idle")).toBeNull();
  });

  it("første måling kunngjør stille tilstander, men ikke at det kjører", () => {
    expect(announceKey(null, "running")).toBeNull();
    expect(announceKey(null, "paused_on_battery")).toBe("indexing.state.paused_on_battery");
  });
});

describe("IndexingSession mot MockBridge", () => {
  it("pause og fortsett gir én melding per endring, ikke per polling", async () => {
    const b = mock({ scanPolls: 50 });
    await b.startScan(true);
    const s = new IndexingSession(b);
    await s.refresh();
    expect(s.state.announce).toBeNull(); // første måling: kjører
    await s.pause();
    expect(s.state.announce).toBe("indexing.state.paused");
    const before = s.state.announce;
    await s.refresh();
    await s.refresh();
    expect(s.state.announce).toBe(before);
    await s.resume();
    expect(s.state.announce).toBe("indexing.state.running");
    await s.cancel();
    expect(s.state.announce).toBe("indexing.live.done");
    s.dispose();
  });

  it("batteri: tilstanden kunngjøres, og Fortsett sendes til kjernen (som lar den overstyre batteripausen)", async () => {
    const b = mock({ scanPolls: 50, onBattery: true });
    await b.startScan(true);
    const spy = vi.spyOn(b, "resumeIndexing").mockResolvedValue();
    const s = new IndexingSession(b);
    await s.refresh();
    expect(s.state.status.state).toBe("paused_on_battery");
    expect(s.state.announce).toBe("indexing.state.paused_on_battery");
    await s.resume();
    expect(spy).toHaveBeenCalledTimes(1);
    expect(s.state.error).toBeNull();
    s.dispose();
  });

  it("opptatt-flagget hindrer dobbeltklikk", async () => {
    const b = mock({ scanPolls: 50 });
    await b.startScan(true);
    const s = new IndexingSession(b);
    await s.refresh();
    const first = s.pause();
    const second = s.pause(); // ignoreres mens første pågår
    await Promise.all([first, second]);
    expect(s.state.error).toBeNull();
    expect(s.state.status.state).toBe("paused");
    s.dispose();
  });
});
