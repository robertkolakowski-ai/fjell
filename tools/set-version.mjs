#!/usr/bin/env node
// set-version: setter appversjon i tauri.conf.json, desktop/package.json og desktop/src-tauri/Cargo.toml.
// Brukes av CI (.github/workflows/beta.yml) FØR bygging: 0.1.<github.run_number>.
// Ingen avhengigheter. Bruk: node tools/set-version.mjs <X.Y.Z> [--root <mappe>] [--dry-run]
// Versjonen må være ren numerisk semver (NSIS godtar ikke pre-release-suffiks).
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
let root = resolve(here, "..");
let dry = false;
let version = null;
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--root") root = resolve(args[++i] ?? "");
  else if (args[i] === "--dry-run") dry = true;
  else if (!version) version = args[i];
  else fail(`uventet argument: ${args[i]}`);
}

function fail(msg) {
  console.error(`set-version: ${msg}`);
  process.exit(1);
}

if (!version || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) {
  fail("bruk: node tools/set-version.mjs <X.Y.Z> [--root <mappe>] [--dry-run] (ren numerisk semver)");
}

// Bytter første «"version": "…"» på toppnivå (to blanke innrykk) og beholder resten av filen byte for byte.
function setJson(rel) {
  const path = join(root, rel);
  const src = readFileSync(path, "utf8");
  const re = /^(  "version"\s*:\s*")[^"]*(")/m;
  if (!re.test(src)) fail(`fant ikke toppnivå-«version» i ${rel}`);
  const out = src.replace(re, `$1${version}$2`);
  if (JSON.parse(out).version !== version) fail(`${rel}: versjonen ble ikke satt`);
  return { path, rel, before: JSON.parse(src).version, out };
}

// Cargo.toml: workspace-versjon der den står ([workspace.package]), ellers [package].
function setCargo(rel) {
  const path = join(root, rel);
  const src = readFileSync(path, "utf8");
  const lines = src.split("\n");
  for (const section of ["[workspace.package]", "[package]"]) {
    let inSec = false;
    for (let i = 0; i < lines.length; i++) {
      const t = lines[i].trim();
      if (t.startsWith("[")) inSec = t === section;
      else if (inSec && /^version\s*=\s*"[^"]*"/.test(t)) {
        const before = t.match(/"([^"]*)"/)[1];
        lines[i] = lines[i].replace(/"[^"]*"/, `"${version}"`);
        return { path, rel, before, out: lines.join("\n") };
      }
    }
  }
  fail(`fant ingen versjonslinje i [workspace.package] eller [package] i ${rel}`);
}

const changes = [
  setJson("desktop/src-tauri/tauri.conf.json"),
  setJson("desktop/package.json"),
  setCargo("desktop/src-tauri/Cargo.toml"),
];
for (const c of changes) {
  console.log(`${c.rel}: ${c.before} -> ${version}${dry ? " (tørrkjøring)" : ""}`);
  if (!dry) writeFileSync(c.path, c.out);
}
