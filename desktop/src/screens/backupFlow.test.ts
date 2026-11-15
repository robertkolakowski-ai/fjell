// Sikkerhetskopi av indeksen (docs/v1/BACKUP.md): angre finnes bare når det finnes en kopi av «før» (trygg modus uten åpen
// indeks gir `safety` = 0), og gjenoppretting kan ikke gå uten avkrysning.
import { describe, expect, it, vi } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import { BackupSession, canUndo } from "./backupFlow";

const mock = () => new MockBridge({ now: () => Date.UTC(2026, 10, 15), integrity: "corrupt" });

describe("canUndo", () => {
  it("krever en sikringskopi", () => {
    expect(canUndo(null)).toBe(false);
    expect(canUndo({ restored: 2, safety: 0, assetCount: 3 })).toBe(false);
    expect(canUndo({ restored: 2, safety: 3, assetCount: 3 })).toBe(true);
  });
});

describe("BackupSession uten kopi av «før»", () => {
  it("angre gjør ingenting når safety er 0", async () => {
    const b = mock();
    vi.spyOn(b, "restoreBackup").mockResolvedValue({ restored: 2, safety: 0, assetCount: 3 });
    const undo = vi.spyOn(b, "undoRestoreBackup");
    const s = new BackupSession(b);
    await s.load();
    s.requestRestore(2);
    s.setAck(true);
    expect(await s.confirmRestore()).toBe(true);
    await s.undoRestore();
    expect(undo).not.toHaveBeenCalled();
    expect(s.state.restored).toEqual({ restored: 2, safety: 0, assetCount: 3 });
  });

  it("feilkode fra kjernen vises som kode, og dialogen lukkes uten at noe er endret", async () => {
    const b = mock();
    vi.spyOn(b, "restoreBackup").mockRejectedValue({ code: "backup_corrupt", detail: "" });
    const s = new BackupSession(b);
    await s.load();
    s.requestRestore(1);
    s.setAck(true);
    expect(await s.confirmRestore()).toBe(false);
    expect(s.state).toMatchObject({ error: "backup_corrupt", confirmId: null, ack: false, restored: null });
  });

  it("ingen gjenoppretting uten avkrysning", async () => {
    const b = mock();
    const spy = vi.spyOn(b, "restoreBackup");
    const s = new BackupSession(b);
    await s.load();
    s.requestRestore(1);
    expect(await s.confirmRestore()).toBe(false);
    expect(spy).not.toHaveBeenCalled();
  });
});
