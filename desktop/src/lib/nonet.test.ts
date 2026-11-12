// «Ingen nettverkskall» (PERSONVERN.md, ARKITEKTUR 2/16): statisk bevis. Frontend har ingen kall ut, CSP tillater
// bare Tauri-IPC, og appens egne Rust-crates bruker ingen nettverks-API. Kjøretidsbeviset er e2e/nonet.spec.ts.
// Testen er bevisst streng: legg ikke til unntak uten å oppdatere docs/PERSONVERN.md.
import { readdirSync, readFileSync, statSync, existsSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";

const here = dirname(fileURLToPath(import.meta.url));
const desktop = join(here, "..", "..");
const repo = join(desktop, "..");

function walk(dir: string, accept: (p: string) => boolean, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    if (name === "node_modules" || name === "target" || name === ".results" || name === "dist") continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, accept, out);
    else if (accept(p)) out.push(p);
  }
  return out;
}

/** Produksjonskode i desktop/src (ikke tester, ikke denne filen). */
const srcFiles = walk(join(desktop, "src"), (p) => /\.(ts|svelte|css|html|js|mjs)$/.test(p) && !/\.test\.ts$/.test(p));

// Navnerom og lenker i kommentarer er ikke nettverkstilgang, men vi tillater ingen URL i det hele tatt unntatt
// W3C-navnerom (xmlns i inline-SVG). Alt annet er et funn.
const ALLOWED_URL = /^https?:\/\/(www\.w3\.org\/(2000\/svg|1999\/xlink|1998\/Math\/MathML|1999\/xhtml)|localhost(:\d+)?)(\/|$|["'`)\s])/;

const FORBIDDEN_API: [string, RegExp][] = [
  ["fetch(", /\bfetch\s*\(/],
  ["XMLHttpRequest", /\bXMLHttpRequest\b/],
  ["WebSocket", /\bWebSocket\b/],
  ["EventSource", /\bEventSource\b/],
  ["sendBeacon", /\bsendBeacon\b/],
  ["navigator.serviceWorker", /\bserviceWorker\b/],
  ["importScripts", /\bimportScripts\b/],
  ["dynamic import av URL", /\bimport\s*\(\s*["'`]https?:/],
  ["Tauri http/shell/fs/opener-plugin", /@tauri-apps\/plugin-(http|shell|fs|opener|updater|websocket)/],
  ["window.open", /\bwindow\.open\s*\(/],
];

describe("frontend: ingen nettverkskall (desktop/src)", () => {
  test("det finnes kildefiler å skanne", () => {
    expect(srcFiles.length).toBeGreaterThan(20);
  });

  test.each(FORBIDDEN_API)("ingen %s", (_name, re) => {
    const hits = srcFiles.filter((f) => re.test(readFileSync(f, "utf8"))).map((f) => relative(desktop, f));
    expect(hits).toEqual([]);
  });

  test("ingen eksterne URL-er (http/https/ws/ftp, protokoll-relative, @import url, CDN)", () => {
    const hits: string[] = [];
    for (const f of srcFiles) {
      const txt = readFileSync(f, "utf8");
      for (const m of txt.matchAll(/\b(?:https?|wss?|ftp):\/\/[^\s"'`)<>]+/g)) {
        if (!ALLOWED_URL.test(m[0] + " ")) hits.push(`${relative(desktop, f)}: ${m[0]}`);
      }
      for (const m of txt.matchAll(/(?:src|href|action|poster)\s*=\s*["']\s*\/\/[^"']+/g)) hits.push(`${relative(desktop, f)}: ${m[0]}`);
      for (const m of txt.matchAll(/url\(\s*["']?\s*(?:https?:)?\/\/[^)]+\)/g)) hits.push(`${relative(desktop, f)}: ${m[0]}`);
      for (const m of txt.matchAll(/@import\s+(?:url\()?["']?(?:https?:)?\/\//g)) hits.push(`${relative(desktop, f)}: ${m[0]}`);
    }
    expect(hits).toEqual([]);
  });

  test("ingen eksterne fonter: font-face bare med lokal kilde, og ingen Google Fonts", () => {
    const hits: string[] = [];
    for (const f of srcFiles) {
      const txt = readFileSync(f, "utf8");
      if (/fonts\.(googleapis|gstatic)\.com|use\.typekit|cdn\.|cdnjs|unpkg|jsdelivr/i.test(txt)) hits.push(relative(desktop, f));
    }
    expect(hits).toEqual([]);
  });

  test("index.html laster bare lokale ressurser", () => {
    const html = readFileSync(join(desktop, "index.html"), "utf8");
    for (const m of html.matchAll(/(?:src|href)\s*=\s*"([^"]+)"/g)) expect(m[1], m[0]).toMatch(/^(\/|\.\/|#)/);
    expect(html).not.toMatch(/https?:\/\//);
  });

  test("ingen nettverks-pakker i package.json (dependencies)", () => {
    const pkg = JSON.parse(readFileSync(join(desktop, "package.json"), "utf8")) as { dependencies?: Record<string, string> };
    const deps = Object.keys(pkg.dependencies ?? {});
    expect(deps.filter((d) => /^(axios|got|node-fetch|ky|superagent|socket\.io|ws|undici|@tauri-apps\/plugin-(http|shell|fs|opener|updater|websocket))/.test(d))).toEqual([]);
    // kjøretidsavhengigheter; ny avhengighet krever oppdatering her og i THIRD_PARTY.md. plugin-dialog = «Bla …» (mappevalg, ingen nettverk)
    expect(deps.sort()).toEqual(["@tauri-apps/api", "@tauri-apps/plugin-dialog"]);
  });

  test("bygget dist (hvis det finnes) har ingen eksterne URL-er eller fetch-kall fra appkode", () => {
    const dist = join(desktop, "dist");
    if (!existsSync(dist)) return;
    const hits: string[] = [];
    for (const f of walk(dist, (p) => /\.(html|js|css)$/.test(p))) {
      const txt = readFileSync(f, "utf8");
      for (const m of txt.matchAll(/\b(?:https?|wss?):\/\/[^\s"'`)<>\\]+/g)) {
        // Svelte-kjøretiden inneholder navnerom og lenker til dokumentasjon (svelte.dev/e/...) i feilmeldinger, uten kall.
        if (/^https?:\/\/(www\.w3\.org\/|svelte\.dev\/|github\.com\/sveltejs\/)/.test(m[0])) continue;
        if (/^https?:\/\/ipc\.localhost/.test(m[0])) continue;
        hits.push(`${relative(desktop, f)}: ${m[0]}`);
      }
    }
    expect(hits).toEqual([]);
  });
});

describe("selvtest: skanneren finner det den skal finne", () => {
  const positives: Record<string, string> = {
    "fetch(": "await fetch('/x')", XMLHttpRequest: "new XMLHttpRequest()", WebSocket: "new WebSocket('ws://x')",
    EventSource: "new EventSource(u)", sendBeacon: "navigator.sendBeacon(u)", "navigator.serviceWorker": "navigator.serviceWorker.register()",
    importScripts: "importScripts('a')", "dynamic import av URL": "import('https://x/y.js')",
    "Tauri http/shell/fs/opener-plugin": "import { fetch } from '@tauri-apps/plugin-http'", "window.open": "window.open('x')",
  };
  test.each(FORBIDDEN_API)("%s slår til på eksempelkode", (name, re) => { expect(re.test(positives[name]), name).toBe(true); });
  test("URL-tillatelsen slipper bare W3C-navnerom og localhost", () => {
    expect(ALLOWED_URL.test('http://www.w3.org/2000/svg" ')).toBe(true);
    expect(ALLOWED_URL.test("https://cdn.example.com/a.js ")).toBe(false);
    expect(ALLOWED_URL.test("https://www.w3.org.evil.com/2000/svg ")).toBe(false);
  });
});

describe("CSP og Tauri-rettigheter", () => {
  const conf = JSON.parse(readFileSync(join(desktop, "src-tauri", "tauri.conf.json"), "utf8")) as { app: { security: { csp: string } } };
  const csp = conf.app.security.csp;
  const dirs = new Map(csp.split(";").map((d) => d.trim()).filter(Boolean).map((d) => { const [n, ...v] = d.split(/\s+/); return [n, v] as const; }));

  test("connect-src er bare Tauri-IPC (ingen 'self', ingen verter, ingen wildcard)", () => {
    expect(dirs.get("connect-src")?.slice().sort()).toEqual(["http://ipc.localhost", "ipc:"]);
  });

  test("ingen direktiv åpner for nettverket: ingen http(s)/ws-kilder, ingen * og ingen unsafe-eval/-inline i script", () => {
    for (const [name, vals] of dirs) {
      if (name === "connect-src") continue;
      for (const v of vals) expect(`${name} ${v}`, csp).not.toMatch(/(https?|wss?):\/\/|^\S+ \*$/);
    }
    expect(dirs.get("script-src")).toEqual(["'self'"]);
    expect(dirs.get("default-src")).toEqual(["'self'"]);
    expect(dirs.get("font-src")).toEqual(["'self'"]);
    expect(dirs.get("img-src")).toEqual(["'self'", "data:"]);
    expect(dirs.get("object-src")).toEqual(["'none'"]);
    expect(dirs.get("frame-ancestors")).toEqual(["'none'"]);
    expect(dirs.get("form-action")).toEqual(["'none'"]);
    expect(dirs.get("base-uri")).toEqual(["'none'"]);
    expect(csp).not.toMatch(/unsafe-eval/);
  });

  // Eneste unntak fra «bare core:»: mappevalg (`dialog:allow-open`). Ikke `dialog:default`, ingen save/message/ask/confirm.
  const DIALOG_OPEN = "dialog:allow-open";

  test("capabilities: ingen http-, shell-, fs-, opener- eller updater-rettigheter", () => {
    const dir = join(desktop, "src-tauri", "capabilities");
    for (const f of readdirSync(dir).filter((n) => n.endsWith(".json"))) {
      const cap = JSON.parse(readFileSync(join(dir, f), "utf8")) as { permissions: (string | { identifier: string })[] };
      const ids = cap.permissions.map((p) => (typeof p === "string" ? p : p.identifier));
      expect(ids.filter((i) => i !== DIALOG_OPEN).filter((i) => /^(http|shell|fs|opener|updater|websocket|process|os|dialog)\b|:allow-(fetch|open|spawn|execute)/.test(i)), f).toEqual([]);
      expect(ids.every((i) => i.startsWith("core:") || i === DIALOG_OPEN), `${f}: ${ids.join(",")}`).toBe(true);
    }
  });

  test("ingen autoupdate i tauri.conf.json", () => {
    const raw = readFileSync(join(desktop, "src-tauri", "tauri.conf.json"), "utf8");
    expect(raw).not.toMatch(/"updater"|"endpoints"|createUpdaterArtifacts/);
  });
});

describe("Rust: appens egne crates bruker ingen nettverks-API", () => {
  const roots = [join(repo, "core", "crates"), join(repo, "core", "testdata", "src"), join(desktop, "src-tauri", "src"), join(desktop, "src-tauri", "shell", "src")];
  // fjell-cli er testsele (ikke levert), men holdes likevel nettverksfri.
  const rsFiles = roots.filter(existsSync).flatMap((r) => walk(r, (p) => p.endsWith(".rs")));
  const NET = /\b(std::net|TcpStream|TcpListener|UdpSocket|reqwest|hyper|ureq|curl::|tungstenite|WinHttp|WinINet|InternetOpen|URLDownloadToFile)\b/;

  test("det finnes Rust-filer å skanne", () => { expect(rsFiles.length).toBeGreaterThan(10); });

  test("ingen std::net, reqwest, hyper, ureq, WinHTTP osv. i kildekoden", () => {
    const hits = rsFiles.filter((f) => NET.test(readFileSync(f, "utf8"))).map((f) => relative(repo, f));
    expect(hits).toEqual([]);
  });

  test("core/Cargo.lock (levert kjerne) inneholder ingen nettverks- eller async-runtime-crates", () => {
    const lock = readFileSync(join(repo, "core", "Cargo.lock"), "utf8");
    const names = [...lock.matchAll(/^name = "([^"]+)"/gm)].map((m) => m[1] as string);
    expect(names.filter((n) => /^(reqwest|hyper|hyper-util|ureq|curl|isahc|surf|attohttpc|tokio|h2|rustls|native-tls|openssl|openssl-sys|tungstenite|tokio-tungstenite|socket2|mio)$/.test(n))).toEqual([]);
  });

  test("egne crates har ingen direkte nettverksavhengighet i Cargo.toml", () => {
    const manifests = [...walk(join(repo, "core", "crates"), (p) => p.endsWith("Cargo.toml")), join(desktop, "src-tauri", "Cargo.toml"), join(desktop, "src-tauri", "shell", "Cargo.toml")].filter(existsSync);
    const hits = manifests.filter((m) => /^\s*(reqwest|hyper|ureq|curl|isahc|surf|attohttpc|tokio|tungstenite|tauri-plugin-(http|shell|fs|opener|updater|websocket))\b/m.test(readFileSync(m, "utf8"))).map((m) => relative(repo, m));
    expect(hits).toEqual([]);
  });
});
