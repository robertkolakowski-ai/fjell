// Diagnose (docs/v1/DIAGNOSE.md): stien vises bare hvis skallet oppgir den, og feil gir kode, aldri tekst.
import { describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import { DiagSession } from "./diagFlow";

const mock = () => new MockBridge({ now: () => Date.UTC(2026, 10, 15) });

describe("DiagSession (lagring)", () => {
  it("har ingen sti når skallet ikke oppgir en, og viser den når den finnes", async () => {
    const b = mock();
    const s = new DiagSession(b);
    await s.load();
    await s.exportReport();
    expect(s.state).toMatchObject({ exported: true, savedPath: null });

    const real = await b.exportDiagnostics();
    b.exportDiagnostics = async () => ({ ...real, savedPath: "C:\\Users\\x\\AppData\\Fjell\\diagnose\\fjell-diagnose-1.txt" }) as typeof real;
    await s.exportReport();
    expect(s.state.savedPath).toContain("fjell-diagnose-1.txt");
  });

  it("ignorerer en tom eller ugyldig sti", async () => {
    const b = mock();
    const real = await b.exportDiagnostics();
    b.exportDiagnostics = async () => ({ ...real, savedPath: 42 }) as unknown as typeof real;
    const s = new DiagSession(b);
    await s.load();
    await s.exportReport();
    expect(s.state.savedPath).toBeNull();
  });

  it("gir feilkode og nullstiller sti ved mislykket lagring", async () => {
    const b = mock();
    const s = new DiagSession(b);
    await s.load();
    b.exportDiagnostics = async () => { throw { code: "diag_failed", message: "" }; };
    await s.exportReport();
    expect(s.state).toMatchObject({ exported: false, savedPath: null, error: "diag_failed", busy: false });
  });

  it("rapporten i forhåndsvisningen har ingen stier", async () => {
    const s = new DiagSession(mock());
    await s.load();
    expect(s.state.report?.text).not.toMatch(/[\\/:]/);
  });
});
