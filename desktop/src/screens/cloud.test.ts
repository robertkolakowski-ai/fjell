import { describe, expect, it } from "vitest";
import { MockBridge } from "../bridge/mock-bridge";
import { cloudFromError, loadCloud } from "./cloud";

describe("loadCloud / cloudFromError", () => {
  it("gir leverandør for skymappe og null ellers", async () => {
    expect(await loadCloud(new MockBridge({ library: "C:\\Users\\a\\OneDrive\\Bilder" }))).toBe("onedrive");
    expect(await loadCloud(new MockBridge({ library: "/u/Dropbox/x" }))).toBe("dropbox");
    expect(await loadCloud(new MockBridge())).toBeNull();
    expect(await loadCloud(new MockBridge({ library: null }))).toBeNull();
  });
  it("leser leverandør fra feilkoden", () => {
    expect(cloudFromError({ code: "cloud_synced_readonly", detail: "icloud" })).toBe("icloud");
    expect(cloudFromError({ code: "cloud_synced_readonly", detail: "" })).toBe("cloud");
    expect(cloudFromError({ code: "core_error", detail: "icloud" })).toBeNull();
    expect(cloudFromError(null)).toBeNull();
  });
});
