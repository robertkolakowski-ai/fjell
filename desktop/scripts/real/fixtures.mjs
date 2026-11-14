// Deterministisk fixture-generator for ekte-app-e2e (docs/mvp/E2E-WINDOWS.md).
// Rene JS-biblioteker (jpeg-js) og node:zlib, ingen native moduler. Fast seed: samme byte-for-byte resultat hver gang.
// Bruk: node scripts/real/fixtures.mjs <målmappe>   (eller importer generate())
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { deflateSync } from "node:zlib";
import jpeg from "jpeg-js";

const SEED = 20260101;
const W = 320, H = 240;

function rng(seed) { // mulberry32
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Fotoaktig syntetisk bilde: fargegradient + tilfeldige blobber + fin tekstur. `detail` 0..1 styrer skarphet. */
function scene(seed, { noise = 0, blur = 0, jitter = 0 } = {}) {
  const r = rng(seed);
  const c0 = [r() * 255, r() * 255, r() * 255], c1 = [r() * 255, r() * 255, r() * 255];
  const blobs = Array.from({ length: 9 }, () => ({ x: r() * W, y: r() * H, rad: 20 + r() * 70, c: [r() * 255, r() * 255, r() * 255] }));
  const px = new Float32Array(W * H * 3);
  const jr = rng(seed ^ 0x9e3779b9);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const t = (x / W + y / H) / 2;
    const o = (y * W + x) * 3;
    for (let k = 0; k < 3; k++) px[o + k] = c0[k] * (1 - t) + c1[k] * t;
    for (const b of blobs) {
      const d = Math.hypot(x - b.x, y - b.y) / b.rad;
      if (d < 1) { const w = (1 - d) * (1 - d); for (let k = 0; k < 3; k++) px[o + k] = px[o + k] * (1 - w) + b.c[k] * w; }
    }
    const tex = blur ? 0 : Math.sin(x * 0.9 + seed) * Math.cos(y * 0.7) * 14;
    for (let k = 0; k < 3; k++) px[o + k] += tex + (noise ? (jr() - 0.5) * noise : 0) + (jitter ? (jr() - 0.5) * jitter : 0);
  }
  let cur = px;
  for (let pass = 0; pass < blur; pass++) { // enkel boksuskarphet
    const nxt = new Float32Array(cur.length);
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) for (let k = 0; k < 3; k++) {
      let s = 0, n = 0;
      for (let dy = -2; dy <= 2; dy++) for (let dx = -2; dx <= 2; dx++) {
        const xx = x + dx, yy = y + dy;
        if (xx >= 0 && yy >= 0 && xx < W && yy < H) { s += cur[(yy * W + xx) * 3 + k]; n++; }
      }
      nxt[(y * W + x) * 3 + k] = s / n;
    }
    cur = nxt;
  }
  const data = Buffer.alloc(W * H * 4);
  for (let i = 0; i < W * H; i++) {
    for (let k = 0; k < 3; k++) data[i * 4 + k] = Math.max(0, Math.min(255, Math.round(cur[i * 3 + k])));
    data[i * 4 + 3] = 255;
  }
  return data;
}

const jpg = (data, q = 88) => Buffer.from(jpeg.encode({ data, width: W, height: H }, q).data);

// PNG for skjermbilde-aktig innhold (flate flater, tekstlinjer), skrevet med node:zlib.
function crc32(buf) {
  let c, crc = 0xffffffff;
  for (let n = 0; n < buf.length; n++) {
    c = (crc ^ buf[n]) & 0xff;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    crc = (crc >>> 8) ^ c;
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function chunk(type, body) {
  const len = Buffer.alloc(4); len.writeUInt32BE(body.length);
  const td = Buffer.concat([Buffer.from(type, "ascii"), body]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}
function screenshotPng() {
  const w = 360, h = 780, rgb = Buffer.alloc(w * h * 3);
  const fill = (x0, y0, x1, y1, c) => { for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) { const o = (y * w + x) * 3; rgb[o] = c[0]; rgb[o + 1] = c[1]; rgb[o + 2] = c[2]; } };
  fill(0, 0, w, h, [248, 248, 250]);
  fill(0, 0, w, 60, [30, 90, 180]);
  const r = rng(SEED + 7);
  for (let i = 0; i < 14; i++) { const y = 90 + i * 48; fill(16, y, w - 16, y + 36, [255, 255, 255]); fill(28, y + 10, 28 + 60 + Math.floor(r() * 180), y + 16, [60, 60, 70]); fill(28, y + 22, 28 + 40 + Math.floor(r() * 120), y + 26, [150, 150, 160]); }
  fill(0, h - 56, w, h, [235, 235, 240]);
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) { raw[y * (w * 3 + 1)] = 0; rgb.copy(raw, y * (w * 3 + 1) + 1, y * w * 3, (y + 1) * w * 3); }
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw, { level: 9 })), chunk("IEND", Buffer.alloc(0))]);
}

/** Korrupt JPEG: gyldig start (SOI + APP0), deretter søppel uten slutt-markør. */
function corruptJpg() {
  const r = rng(SEED + 99);
  const head = Buffer.from([0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
  return Buffer.concat([head, Buffer.from(Array.from({ length: 3000 }, () => Math.floor(r() * 256)))]);
}

/** Filnavn bærer prefikset «fjelltest-» så de lett finnes i Windows Papirkurv. */
export const DUP_GROUPS = 3;

/**
 * Lager testbiblioteket under `root/photos` og `root/manifest.json` (utenfor bildemappen, så skann ikke ser den).
 * Returnerer manifestet: { photosDir, files: [{ rel, sha256, role, group? }], dupGroups }.
 */
export function generate(root) {
  const photos = join(root, "photos");
  rmSync(root, { recursive: true, force: true });
  mkdirSync(join(photos, "ferie"), { recursive: true });
  mkdirSync(join(photos, "kopier"), { recursive: true });
  const files = [];
  const put = (rel, buf, role, group) => {
    writeFileSync(join(photos, rel), buf);
    files.push({ rel, sha256: createHash("sha256").update(buf).digest("hex"), role, ...(group ? { group } : {}) });
  };
  // Eksakte duplikatpar (identiske bytes, ulike navn og mapper)
  for (let g = 1; g <= DUP_GROUPS; g++) {
    const b = jpg(scene(SEED + g * 11));
    put(`ferie/fjelltest-dup${g}-a.jpg`, b, "dup", `dup${g}`);
    put(`kopier/fjelltest-dup${g}-b.jpg`, b, "dup", `dup${g}`);
  }
  // Serie med like bilder (samme motiv, små forskjeller), ikke byte-identiske
  for (let i = 1; i <= 4; i++) put(`ferie/fjelltest-serie-${i}.jpg`, jpg(scene(SEED + 500, { jitter: 6 + i * 2 }), 85), "serie");
  // Tydelig uskarpt bilde
  put("ferie/fjelltest-uskarp.jpg", jpg(scene(SEED + 600, { blur: 6 }), 90), "uskarp");
  // Skjermbilde-aktig PNG
  put("fjelltest-skjermbilde.png", screenshotPng(), "skjermbilde");
  // Unike bilder
  for (let i = 1; i <= 5; i++) put(`ferie/fjelltest-unik-${i}.jpg`, jpg(scene(SEED + 700 + i * 13)), "unik");
  // Korrupt fil
  put("fjelltest-korrupt.jpg", corruptJpg(), "korrupt");
  const manifest = { seed: SEED, photosDir: photos, dupGroups: DUP_GROUPS, files };
  writeFileSync(join(root, "manifest.json"), JSON.stringify(manifest, null, 2));
  return manifest;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const dir = process.argv[2];
  if (!dir) { console.error("bruk: node scripts/real/fixtures.mjs <målmappe>"); process.exit(2); }
  const m = generate(dir);
  console.log(`${m.files.length} filer i ${m.photosDir}`);
}
