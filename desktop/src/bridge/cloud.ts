// Speiler fjell-fs `cloud_provider` (core/crates/fjell-fs/src/cloud.rs) for MockBridge og tester. Samme regler:
// hele sti-komponenter (delt på / og \), case-insensitivt, \\?\UNC\ og \\?\ fjernes først.
// Et sikkerhetsnett basert på kjente mappenavn, ikke en garanti.
import type { CloudCode } from "./types";

function classify(component: string): CloudCode | null {
  const c = component.toLowerCase();
  if (c === "icloud photos" || c === "iclouddrive" || c === "icloud drive" || c === "icloud-bilder") return "icloud";
  if (c === "onedrive" || c.startsWith("onedrive -") || c.startsWith("onedrive-")) return "onedrive";
  if (c === "dropbox" || c.startsWith("dropbox (")) return "dropbox";
  return null;
}

export function cloudProvider(path: string): CloudCode | null {
  const bare = path.startsWith("\\\\?\\UNC\\") ? path.slice(8) : path.startsWith("\\\\?\\") ? path.slice(4) : path;
  for (const part of bare.split(/[\\/]/)) {
    if (part === "") continue;
    const hit = classify(part);
    if (hit) return hit;
  }
  return null;
}
