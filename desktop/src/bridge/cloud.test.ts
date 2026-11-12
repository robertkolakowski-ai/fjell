// Speiler enhetstestene i fjell-fs/src/cloud.rs: samme deteksjonsregler i TS.
import { describe, expect, it } from "vitest";
import { cloudProvider } from "./cloud";

describe("cloudProvider (speil av fjell-fs)", () => {
  it("kjenner kjente mappenavn uavhengig av store/små bokstaver", () => {
    expect(cloudProvider("/Users/a/OneDrive/Bilder")).toBe("onedrive");
    expect(cloudProvider("/u/ONEDRIVE")).toBe("onedrive");
    expect(cloudProvider("/u/dropbox/x")).toBe("dropbox");
    expect(cloudProvider("/u/iCloudDrive/x")).toBe("icloud");
    expect(cloudProvider("/u/ICLOUD PHOTOS")).toBe("icloud");
    expect(cloudProvider("/u/iCloud Drive")).toBe("icloud");
    expect(cloudProvider("/u/iCloud-Bilder")).toBe("icloud");
  });
  it("kjenner varianter og Windows-stier", () => {
    expect(cloudProvider("C:\\Users\\a\\OneDrive - Firma AS\\Bilder")).toBe("onedrive");
    expect(cloudProvider("C:\\Users\\a\\OneDrive-Personal\\x")).toBe("onedrive");
    expect(cloudProvider("C:\\Users\\a\\Dropbox (Personal)\\x")).toBe("dropbox");
    expect(cloudProvider("\\\\?\\C:\\Users\\a\\Dropbox\\x")).toBe("dropbox");
    expect(cloudProvider("\\\\?\\UNC\\server\\share\\OneDrive\\x")).toBe("onedrive");
  });
  it("gir ingen falske treff på deler av navn", () => {
    for (const p of ["C:\\Bilder", "/u/MyOneDriveNotes", "/u/OneDriveBackup", "/u/NotDropbox", "/u/Dropbox2", ""]) {
      expect(cloudProvider(p), p).toBeNull();
    }
  });
});
