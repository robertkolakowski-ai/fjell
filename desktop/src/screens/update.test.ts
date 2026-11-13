import { get } from "svelte/store";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { MockBridge, type MockOptions } from "../bridge/mock-bridge";
import { pendingUpdate, runStartupCheck, UpdateSession } from "./updateFlow";
import { memoryAttempts, olderThan, type AttemptStore } from "./updateAttempt";
import { initialUpdate, installBlocked, isScanning, progressPercent, reduceUpdate, showManual, type UpdateView } from "./updateState";

beforeEach(() => pendingUpdate.set(null));

async function session(opts: MockOptions = {}, attempts: AttemptStore = memoryAttempts(), version?: string, clipboard = vi.fn().mockResolvedValue(undefined)) {
  const b = new MockBridge(opts);
  if (version) vi.spyOn(b, "getAppVersion").mockResolvedValue(version);
  const s = new UpdateSession(b, undefined, attempts, clipboard);
  await s.load();
  return { b, s, attempts, clipboard };
}

describe("oppdatering: reducer", () => {
  const avail: UpdateView = { ...initialUpdate, status: "available", found: { available: true, version: "0.1.9" } };
  it("starter i ro: ingen sjekk, auto av", () => {
    expect(initialUpdate).toMatchObject({ status: "idle", auto: false, found: null, scanning: false });
  });
  it("sjekk gir none eller available, feil gir error", () => {
    const c = reduceUpdate(initialUpdate, { type: "checking" });
    expect(reduceUpdate(c, { type: "checked", result: { available: false } }).status).toBe("none");
    expect(reduceUpdate(c, { type: "checked", result: { available: true, version: "1.2.3" } })).toMatchObject({ status: "available", found: { version: "1.2.3" } });
    expect(reduceUpdate(c, { type: "checkFailed", error: { code: "x", detail: "" } }).status).toBe("error");
  });
  it("resultat uten pågående sjekk ignoreres", () => {
    expect(reduceUpdate(initialUpdate, { type: "checked", result: { available: true, version: "1" } })).toBe(initialUpdate);
  });
  it("ny sjekk mens en sjekk eller installasjon pågår ignoreres", () => {
    const c = reduceUpdate(initialUpdate, { type: "checking" });
    expect(reduceUpdate(c, { type: "checking" })).toBe(c);
    const i = reduceUpdate(avail, { type: "installing" });
    expect(reduceUpdate(i, { type: "checking" })).toBe(i);
  });
  it("installasjon starter bare fra available/installFailed, og aldri mens skanning pågår", () => {
    expect(reduceUpdate(initialUpdate, { type: "installing" })).toBe(initialUpdate);
    const scanning = reduceUpdate(avail, { type: "scan", scanning: true });
    expect(installBlocked(scanning)).toBe(true);
    expect(reduceUpdate(scanning, { type: "installing" })).toBe(scanning);
    expect(reduceUpdate(avail, { type: "installing" }).status).toBe("installing");
  });
  it("fremdrift, omstart og feil", () => {
    const i = reduceUpdate(avail, { type: "installing" });
    expect(reduceUpdate(i, { type: "progress", done: 5, total: 10 })).toMatchObject({ done: 5, total: 10 });
    expect(reduceUpdate(i, { type: "restarting" }).status).toBe("restarting");
    const f = reduceUpdate(i, { type: "installFailed", error: { code: "update_install_failed", detail: "" } });
    expect(f.status).toBe("installFailed");
    expect(reduceUpdate(f, { type: "installing" }).status).toBe("installing"); // prøv igjen
  });
  it("skann pågår bare når det kjører (pauset teller ikke)", () => {
    expect(isScanning({ scanPhase: "analyzing", scanPaused: false })).toBe(true);
    expect(isScanning({ scanPhase: "analyzing", scanPaused: true })).toBe(false);
    expect(isScanning({ scanPhase: "done", scanPaused: false })).toBe(false);
  });
  it("prosent: begrenset til 0-100, null uten total", () => {
    expect(progressPercent(5, 10)).toBe(50);
    expect(progressPercent(20, 10)).toBe(100);
    expect(progressPercent(1, null)).toBeNull();
    expect(progressPercent(1, 0)).toBeNull();
  });
});

describe("oppdatering: flyt", () => {
  it("load leser versjon og auto uten å sjekke etter oppdatering", async () => {
    const b = new MockBridge({ update: "available" });
    const check = vi.spyOn(b, "checkForUpdate");
    const s = new UpdateSession(b);
    await s.load();
    expect(s.state).toMatchObject({ version: "dev", auto: false, status: "idle" });
    expect(check).not.toHaveBeenCalled();
  });
  it("check uten ny versjon", async () => {
    const { s } = await session();
    await s.check();
    expect(s.state.status).toBe("none");
    expect(get(pendingUpdate)).toBeNull();
  });
  it("check med ny versjon setter banner-data, men installerer ikke", async () => {
    const { b, s } = await session({ update: "available" });
    const install = vi.spyOn(b, "installUpdate");
    await s.check();
    expect(s.state).toMatchObject({ status: "available", found: { version: "0.1.99" } });
    expect(get(pendingUpdate)).toEqual({ version: "0.1.99" });
    expect(install).not.toHaveBeenCalled();
  });
  it("check som feiler (frakoblet) gir error og ingen banner", async () => {
    const { s } = await session({ update: "error" });
    await s.check();
    expect(s.state).toMatchObject({ status: "error", error: { code: "update_check_failed" } });
    expect(get(pendingUpdate)).toBeNull();
  });
  it("install: fremdrift, deretter omstart", async () => {
    const { b, s } = await session({ update: "available" });
    const install = vi.spyOn(b, "installUpdate");
    await s.check();
    await s.install();
    expect(install).toHaveBeenCalledTimes(1);
    expect(s.state.status).toBe("restarting");
    expect(s.state).toMatchObject({ done: 24_000_000, total: 24_000_000 });
  });
  it("install uten funnet oppdatering gjør ingenting", async () => {
    const { b, s } = await session({ update: "available" });
    const install = vi.spyOn(b, "installUpdate");
    await s.install();
    expect(install).not.toHaveBeenCalled();
  });
  it("install blokkeres mens skanning pågår, også om skannet startet etter sjekken", async () => {
    const { b, s } = await session({ update: "available", scanPolls: 1000 });
    const install = vi.spyOn(b, "installUpdate");
    await s.check();
    await b.startScan(false);
    expect(s.state.scanning).toBe(false); // tilstanden er ennå ikke lest på nytt
    await s.install();
    expect(s.state).toMatchObject({ status: "available", scanning: true });
    expect(install).not.toHaveBeenCalled();
  });
  it("install åpnes igjen når skannet er pauset eller avbrutt", async () => {
    const { b, s } = await session({ update: "available", scanPolls: 1000 });
    await s.check();
    await b.startScan(false);
    await s.refreshScan();
    expect(installBlocked(s.state)).toBe(true);
    await b.pauseScan();
    await s.refreshScan();
    expect(installBlocked(s.state)).toBe(false);
    await s.install();
    expect(s.state.status).toBe("restarting");
  });
  it("install som feiler gir installFailed og kan prøves igjen", async () => {
    const { b, s } = await session({ update: "available" });
    const fail = vi.spyOn(b, "installUpdate").mockRejectedValueOnce({ code: "update_install_failed", detail: "x" });
    await s.check();
    await s.install();
    expect(s.state.status).toBe("installFailed");
    await s.install();
    expect(fail).toHaveBeenCalledTimes(2);
    expect(s.state.status).toBe("restarting");
  });
  it("auto-bryteren lagres i bridge; feil lar valget stå", async () => {
    const { b, s } = await session();
    await s.setAuto(true);
    expect(s.state.auto).toBe(true);
    expect(await b.getAutoUpdate()).toBe(true);
    vi.spyOn(b, "setAutoUpdate").mockRejectedValueOnce(new Error("x"));
    await s.setAuto(false);
    expect(s.state).toMatchObject({ auto: true, autoFailed: true, autoSaving: false });
  });
  it("dispose stopper videre tilstandsendringer", async () => {
    const { s } = await session();
    s.dispose();
    await s.check();
    expect(s.state.status).toBe("idle");
  });
});

describe("oppdatering: reserveveien (manuell nedlasting)", () => {
  it("versjonssammenligning: bare rene tallversjoner, «dev» er aldri eldre", () => {
    expect(olderThan("0.1.9", "0.1.10")).toBe(true);
    expect(olderThan("0.1.10", "0.1.10")).toBe(false);
    expect(olderThan("0.2.0", "0.1.10")).toBe(false);
    expect(olderThan("0.1", "0.1.1")).toBe(true);
    expect(olderThan("dev", "0.1.10")).toBe(false);
    expect(olderThan("0.1.9", "tull")).toBe(false);
  });
  it("load leser adressen fra bridge; reserveveien vises ikke uten feil", async () => {
    const { s } = await session({ update: "available" });
    expect(s.state.downloadUrl).toMatch(/releases\/tag\/beta$/);
    expect(showManual(s.state)).toBe(false);
    await s.check();
    expect(showManual(s.state)).toBe(false);
  });
  it("installasjon som kaster: reserveveien vises, merket ryddes, og lenken kan kopieres", async () => {
    const { s, attempts, clipboard } = await session({ update: "fail" });
    await s.check();
    await s.install();
    expect(s.state).toMatchObject({ status: "installFailed", error: { code: "update_install_failed" } });
    expect(showManual(s.state)).toBe(true);
    expect(attempts.get()).toBeNull();
    expect(await s.copyLink()).toBe(true);
    expect(clipboard).toHaveBeenCalledWith(s.state.downloadUrl);
    expect(s.state.copied).toBe("ok");
  });
  it("installasjon som stopper (update_install_stalled) gir samme reservevei", async () => {
    const { b, s } = await session({ update: "available" });
    vi.spyOn(b, "installUpdate").mockRejectedValueOnce({ code: "update_install_stalled", detail: "" });
    await s.check();
    await s.install();
    expect(s.state).toMatchObject({ status: "installFailed", error: { code: "update_install_stalled" } });
    expect(showManual(s.state)).toBe(true);
  });
  it("kopiering som feiler gir melding, og lenken står fortsatt", async () => {
    const { s } = await session({ update: "fail" }, memoryAttempts(), undefined, vi.fn().mockRejectedValue(new Error("nektet")));
    await s.check();
    await s.install();
    expect(await s.copyLink()).toBe(false);
    expect(s.state).toMatchObject({ copied: "failed", status: "installFailed" });
    expect(showManual(s.state)).toBe(true);
  });
  it("uten adresse (bridge feiler) vises ingen reservevei, men feilmeldingen står", async () => {
    const b = new MockBridge({ update: "fail" });
    vi.spyOn(b, "getUpdateDownloadUrl").mockRejectedValue({ code: "internal", detail: "" });
    const s = new UpdateSession(b, undefined, memoryAttempts());
    await s.load();
    await s.check();
    await s.install();
    expect(s.state.status).toBe("installFailed");
    expect(showManual(s.state)).toBe(false);
    expect(await s.copyLink()).toBe(false);
  });
  it("install setter merket før installasjonen starter (appen avsluttes av installeren på Windows)", async () => {
    const { b, s, attempts } = await session({ update: "available" });
    let during: string | null = "ikke lest";
    vi.spyOn(b, "installUpdate").mockImplementationOnce(async () => { during = attempts.get(); });
    await s.check();
    await s.install();
    expect(during).toBe("0.1.99");
  });
  it("neste start med samme gamle versjon: «ikke fullført» og reserveveien vises; nytt forsøk skjuler den", async () => {
    const { s, attempts } = await session({ update: "available" }, memoryAttempts("0.1.99"), "0.1.98");
    expect(s.state).toMatchObject({ incomplete: "0.1.99", version: "0.1.98", status: "idle" });
    expect(showManual(s.state)).toBe(true);
    expect(attempts.get()).toBe("0.1.99");
    await s.check();
    expect(showManual(s.state)).toBe(true);
    await s.install();
    expect(s.state.incomplete).toBeNull();
    expect(showManual(s.state)).toBe(false);
  });
  it("neste start med ny versjon (installasjonen lyktes, også manuelt til nyere): merket ryddes, ingen melding", async () => {
    for (const v of ["0.1.99", "0.1.100"]) {
      const { s, attempts } = await session({}, memoryAttempts("0.1.99"), v);
      expect(s.state.incomplete).toBeNull();
      expect(showManual(s.state)).toBe(false);
      expect(attempts.get()).toBeNull();
    }
  });
  it("utenfor installert app («dev»): merket ryddes uten melding", async () => {
    const { s, attempts } = await session({}, memoryAttempts("0.1.99"));
    expect(s.state.incomplete).toBeNull();
    expect(attempts.get()).toBeNull();
  });
});

describe("oppdatering: stille sjekk ved oppstart", () => {
  it("auto av (standard): ingen sjekk, ingen banner", async () => {
    const b = new MockBridge({ update: "available" });
    const check = vi.spyOn(b, "checkForUpdate");
    expect(await runStartupCheck(b)).toBeNull();
    expect(check).not.toHaveBeenCalled();
    expect(get(pendingUpdate)).toBeNull();
  });
  it("auto på og ny versjon: banner-data satt, men aldri installasjon", async () => {
    const b = new MockBridge({ update: "available", autoUpdate: true });
    const check = vi.spyOn(b, "checkForUpdate");
    const install = vi.spyOn(b, "installUpdate");
    expect(await runStartupCheck(b)).toMatchObject({ available: true });
    expect(check).toHaveBeenCalledTimes(1);
    expect(install).not.toHaveBeenCalled();
    expect(get(pendingUpdate)).toEqual({ version: "0.1.99" });
  });
  it("auto på uten ny versjon: ingen banner", async () => {
    expect(await runStartupCheck(new MockBridge({ autoUpdate: true }))).toEqual({ available: false });
    expect(get(pendingUpdate)).toBeNull();
  });
  it("auto på og sjekk feiler: stille, ingen banner, ingen unntak", async () => {
    expect(await runStartupCheck(new MockBridge({ update: "error", autoUpdate: true }))).toBeNull();
    expect(get(pendingUpdate)).toBeNull();
  });
});
