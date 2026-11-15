// Robusthet i broen (docs/v1/ROBUSTHET-KONTRAKT.md): TauriBridge kaller riktige kommandoer; MockBridge har kontraktens semantikk
// (kopier, avkrysning, angre, ødelagt kopi, indeksering som går/pauses/fortsetter, diagnose uten stier).
import { describe, expect, it, vi } from "vitest";
import { MockBridge } from "./mock-bridge";
import { TauriBridge, normalizeError } from "./tauri-bridge";
import { ROBUST_ERROR_CODES, type IndexingSettings } from "./types-robust";
import type { BridgeError } from "./types";

const NOW = Date.UTC(2026, 10, 15);
const mock = (o: ConstructorParameters<typeof MockBridge>[0] = {}) => new MockBridge({ now: () => NOW, ...o });
const code = async (p: Promise<unknown>): Promise<string> => {
  try { await p; return "ok"; } catch (e) { return (e as BridgeError).code; }
};

describe("TauriBridge: robusthetskommandoer", () => {
  it("hver metode kaller kommandoen med samme navn og argumenter som skallet forventer", async () => {
    const call = vi.fn().mockResolvedValue(undefined);
    const b = new TauriBridge(call);
    const settings: IndexingSettings = { threads: 2, priority: "normal", pauseOnBattery: false };
    await b.listBackups();
    await b.createBackup();
    await b.restoreBackup(2, true);
    await b.undoRestoreBackup(5);
    await b.checkIntegrity();
    await b.getIndexingStatus();
    await b.pauseIndexing();
    await b.resumeIndexing();
    await b.cancelIndexing();
    await b.getIndexingSettings();
    await b.setIndexingSettings(settings);
    await b.previewDiagnostics();
    await b.exportDiagnostics();
    expect(call.mock.calls).toEqual([
      ["backup_list"], ["backup_create"], ["backup_restore", { id: 2, acknowledged: true }], ["backup_restore_undo", { safety: 5 }],
      ["integrity_check"], ["indexing_status"], ["indexing_pause"], ["indexing_resume"], ["indexing_cancel"],
      ["get_indexing_settings"], ["set_indexing_settings", { settings }], ["diag_preview"], ["diag_export"],
    ]);
  });

  it("løfter robusthetskoder fra detail til code", () => {
    for (const c of ROBUST_ERROR_CODES) {
      expect(normalizeError({ code: "core_error", detail: c })).toEqual({ code: c, detail: "" });
    }
    expect(normalizeError({ code: "core_error", detail: "noe_annet" })).toEqual({ code: "core_error", detail: "noe_annet" });
  });
});

describe("MockBridge: sikkerhetskopi av indeksen", () => {
  it("lister kopier nyeste først og tar nye kopier", async () => {
    const b = mock();
    const before = await b.listBackups();
    expect(before.map((x) => x.kind)).toEqual(["manual", "auto"]);
    const made = await b.createBackup();
    expect(made.kind).toBe("manual");
    const after = await b.listBackups();
    expect(after[0].id).toBe(made.id);
    expect(after).toHaveLength(3);
  });

  it("gjenoppretting krever avkrysning og tar en pre_restore-kopi som kan angres", async () => {
    const b = mock({ integrity: "corrupt" });
    expect(await code(b.restoreBackup(2, false))).toBe("backup_ack_required");
    expect(await b.checkIntegrity()).toBe("corrupt");
    const r = await b.restoreBackup(2, true);
    expect(r.restored).toBe(2);
    expect(await b.checkIntegrity()).toBe("ok");
    const safety = (await b.listBackups()).find((x) => x.id === r.safety);
    expect(safety).toMatchObject({ kind: "pre_restore", integrity: "corrupt" });
    // Angre setter indeksen tilbake slik den var (også når den var ødelagt).
    await b.undoRestoreBackup(r.safety);
    expect(await b.checkIntegrity()).toBe("corrupt");
    // Kopiene forsvinner aldri stille.
    expect((await b.listBackups()).length).toBe(3);
  });

  it("feilkoder: ukjent kopi, angre av noe som ikke er en sikkerhetskopi, ødelagt indeks kan ikke kopieres, skanning pågår", async () => {
    const b = mock();
    expect(await code(b.restoreBackup(99, true))).toBe("backup_not_found");
    expect(await code(b.undoRestoreBackup(1))).toBe("backup_not_found");
    const sick = mock({ integrity: "unreadable" });
    expect(await code(sick.createBackup())).toBe("backup_failed");
    const busy = mock({ scanPolls: 5 });
    await busy.startScan(true);
    expect(await code(busy.restoreBackup(2, true))).toBe("index_busy");
  });
});

describe("MockBridge: indeksering", () => {
  it("er ledig uten skann, går, pauses og fortsetter, og kan avbrytes", async () => {
    const b = mock({ scanPolls: 50 });
    expect((await b.getIndexingStatus()).state).toBe("idle");
    expect(await code(b.pauseIndexing())).toBe("indexing_not_running");
    await b.startScan(true);
    const run = await b.getIndexingStatus();
    expect(run).toMatchObject({ state: "running", phase: "analyzing" });
    expect(run.done).toBeGreaterThan(0);
    expect(run.total).toBeGreaterThan(run.done);
    expect(run.etaSecs).not.toBeNull();
    await b.pauseIndexing();
    expect(await b.getIndexingStatus()).toMatchObject({ state: "paused", ratePerSec: 0, etaSecs: null });
    await b.resumeIndexing();
    expect((await b.getIndexingStatus()).state).toBe("running");
    await b.cancelIndexing();
    expect((await b.getIndexingStatus()).state).toBe("idle");
    expect(await code(b.cancelIndexing())).toBe("indexing_not_running");
  });

  it("pauser på batteri bare når innstillingen er på, og skannet står stille", async () => {
    const b = mock({ scanPolls: 1, onBattery: true });
    await b.startScan(true);
    expect((await b.getIndexingStatus()).state).toBe("paused_on_battery");
    await b.getState();
    await b.getState();
    expect((await b.getState()).scanPhase).toBe("analyzing");
    await b.setIndexingSettings({ threads: 0, priority: "low", pauseOnBattery: false });
    expect((await b.getIndexingStatus()).state).toBe("running");
  });

  it("innstillinger har standardverdiene og avviser ugyldige", async () => {
    const b = mock();
    expect(await b.getIndexingSettings()).toEqual({ threads: 0, priority: "low", pauseOnBattery: true });
    expect(await b.setIndexingSettings({ threads: 8, priority: "normal", pauseOnBattery: false })).toEqual({ threads: 8, priority: "normal", pauseOnBattery: false });
    expect(await code(b.setIndexingSettings({ threads: 65, priority: "low", pauseOnBattery: true }))).toBe("indexing_invalid_settings");
    expect(await code(b.setIndexingSettings({ threads: 1, priority: "turbo" as never, pauseOnBattery: true }))).toBe("indexing_invalid_settings");
    expect((await b.getIndexingSettings()).threads).toBe(8);
  });
});

describe("MockBridge: diagnose", () => {
  it("rapporten har bare koder og tall, aldri stier eller filnavn", async () => {
    const b = mock({ library: "C:\\Brukere\\ola\\Bilder" });
    const r = await b.previewDiagnostics();
    expect(r.format).toBe(1);
    expect(r.metrics.map((m) => m.code)).toContain("assets_active");
    for (const line of r.text.trim().split("\n")) expect(line).toMatch(/^[a-z_]+=[a-z0-9_]+$/);
    expect(r.text).not.toMatch(/[\\/:]|ola|Bilder/);
    expect(await b.exportDiagnostics()).toEqual(r);
  });
});
