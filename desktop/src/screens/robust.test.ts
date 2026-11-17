// Flytene for sikkerhetskopi og indeksering (docs/v1/ROBUSTHET-KONTRAKT.md): avkrysning er eneste vei til gjenoppretting.
import { describe, expect, it, vi } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import { BackupSession } from "./backupFlow";
import { DiagSession } from "./diagFlow";
import { etaMinutes, IDLE_STATUS, IndexingSession, IndexingSettingsSession } from "./indexingFlow";
import { robustErrorKey } from "./robustErrors";

const mock = (o: ConstructorParameters<typeof MockBridge>[0] = {}) => new MockBridge({ now: () => Date.UTC(2026, 10, 15), ...o });

describe("BackupSession", () => {
  it("laster kopier og integritet", async () => {
    const s = new BackupSession(mock({ integrity: "degraded" }));
    await s.load();
    expect(s.state).toMatchObject({ phase: "ready", integrity: "degraded" });
    expect(s.state.backups).toHaveLength(2);
  });

  it("gjenoppretter ikke uten avkrysning, og kaller bridge først etter avkrysning", async () => {
    const b = mock({ integrity: "corrupt" });
    const spy = vi.spyOn(b, "restoreBackup");
    const s = new BackupSession(b);
    await s.load();
    s.requestRestore(2);
    expect(await s.confirmRestore()).toBe(false);
    expect(spy).not.toHaveBeenCalled();
    s.setAck(true);
    expect(await s.confirmRestore()).toBe(true);
    expect(spy).toHaveBeenCalledWith(2, true);
    expect(s.state).toMatchObject({ integrity: "ok", confirmId: null, ack: false });
    expect(s.state.restored).not.toBeNull();
  });

  it("avbrutt dialog nullstiller avkrysningen, og angre setter indeksen tilbake", async () => {
    const changed = vi.fn();
    const s = new BackupSession(mock({ integrity: "corrupt" }), () => undefined, changed);
    await s.load();
    s.requestRestore(1);
    s.setAck(true);
    s.cancelRestore();
    expect(s.state).toMatchObject({ confirmId: null, ack: false });
    s.requestRestore(1);
    s.setAck(true);
    await s.confirmRestore();
    await s.undoRestore();
    expect(s.state).toMatchObject({ restored: null, integrity: "corrupt" });
    expect(changed).toHaveBeenCalledTimes(2);
  });

  it("feil blir en kode, ikke tekst", async () => {
    const s = new BackupSession(mock({ integrity: "unreadable" }));
    await s.load();
    await s.create();
    expect(s.state.error).toBe("backup_failed");
    expect(robustErrorKey(s.state.error)).toBe("error.backup_failed");
    expect(robustErrorKey("noe_ukjent")).toBe("err.generic");
    expect(robustErrorKey(null)).toBe("err.generic");
  });
});

describe("IndexingSession", () => {
  it("styrer indekseringen og viser statusen etterpå", async () => {
    const b = mock({ scanPolls: 20 });
    await b.startScan(true);
    const s = new IndexingSession(b);
    await s.refresh();
    expect(s.state).toMatchObject({ loaded: true });
    expect(s.state.status.state).toBe("running");
    await s.pause();
    expect(s.state.status.state).toBe("paused");
    await s.resume();
    await s.cancel();
    expect(s.state.status).toMatchObject({ state: "idle" });
    await s.pause();
    expect(s.state.error).toBe("indexing_not_running");
    s.dispose();
  });

  it("gjenstående tid rundes opp til minutter", () => {
    expect(etaMinutes({ ...IDLE_STATUS, etaSecs: null })).toBeNull();
    expect(etaMinutes({ ...IDLE_STATUS, etaSecs: 5 })).toBe(1);
    expect(etaMinutes({ ...IDLE_STATUS, etaSecs: 61 })).toBe(2);
  });

  it("innstillinger lagres og viser det kjernen svarte", async () => {
    const s = new IndexingSettingsSession(mock());
    await s.load();
    expect(s.state.settings).toEqual({ threads: 0, priority: "low", pauseOnBattery: true });
    await s.update({ priority: "normal" });
    expect(s.state.settings?.priority).toBe("normal");
    await s.update({ threads: 99 });
    expect(s.state.error).toBe("indexing_invalid_settings");
    expect(s.state.settings?.threads).toBe(0);
  });
});

describe("DiagSession", () => {
  it("viser forhåndsvisning og lager rapport", async () => {
    const s = new DiagSession(mock());
    await s.load();
    expect(s.state.report?.text).toContain("integrity=ok");
    await s.exportReport();
    expect(s.state).toMatchObject({ exported: true, busy: false, error: null });
  });
});
