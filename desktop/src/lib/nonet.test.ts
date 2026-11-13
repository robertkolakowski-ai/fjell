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
  // plugin-updater har eget unntak (beta-kanalen, M9): bare i tauri-bridge.ts, se «oppdatering» nedenfor.
  ["Tauri http/shell/fs/opener-plugin", /@tauri-apps\/plugin-(http|shell|fs|opener|websocket)/],
  ["window.open", /\bwindow\.open\s*\(/],
];

describe("frontend: ingen nettverkskall (desktop/src)", () => {
  test("det finnes kildefiler å skanne", () => {
    expect(srcFiles.length).toBeGreaterThan(20);
  });

  // Unntak (beta-kanalen, M9, docs/PERSONVERN.md «Oppdateringssjekk»): oppdateringspluginen får brukes i én fil, og bare via
  // dynamisk import slik at ingenting lastes før brukeren trykker «Se etter oppdatering» (eller auto er på). Rust-pluginen
  // gjør selve forespørselen; frontend har fortsatt ingen fetch/XHR/WebSocket.
  const UPDATE_PLUGIN = /@tauri-apps\/plugin-(updater|process)/;
  const UPDATE_FILE = "src/bridge/tauri-bridge.ts";
  test("oppdatering: plugin-updater/-process brukes bare i tauri-bridge.ts, og bare via dynamisk import()", () => {
    const hits = srcFiles.filter((f) => UPDATE_PLUGIN.test(readFileSync(f, "utf8"))).map((f) => relative(desktop, f).split("\\").join("/"));
    expect(hits).toEqual([UPDATE_FILE]);
    const txt = readFileSync(join(desktop, UPDATE_FILE), "utf8");
    expect(txt).not.toMatch(/^\s*import\b[^(\n]*from\s+["']@tauri-apps\/plugin-(updater|process)/m);
    expect(txt).toMatch(/await import\("@tauri-apps\/plugin-updater"\)/);
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
    expect(deps.filter((d) => /^(axios|got|node-fetch|ky|superagent|socket\.io|ws|undici|@tauri-apps\/plugin-(http|shell|fs|opener|websocket))/.test(d))).toEqual([]);
    // Kjøretidsavhengigheter, nøyaktig disse: api; plugin-dialog = «Bla …» (mappevalg, ingen nettverk); plugin-updater/-process = beta-kanalen (M9).
    // Ny avhengighet krever oppdatering her og i THIRD_PARTY.md.
    expect(deps.sort()).toEqual(["@tauri-apps/api", "@tauri-apps/plugin-dialog", "@tauri-apps/plugin-process", "@tauri-apps/plugin-updater"]);
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

  // Nøyaktig disse rettighetene utover core:*, ingenting annet: mappevalg (`dialog:allow-open`; ikke `dialog:default`, ingen
  // save/message/ask/confirm) og beta-kanalen (M9): sjekk/installer og omstart.
  const DIALOG_OPEN = "dialog:allow-open";
  const UPDATE_PERMISSIONS = ["updater:default", "process:allow-restart"];
  const EXTRA_PERMISSIONS = [DIALOG_OPEN, ...UPDATE_PERMISSIONS];
  test("capabilities: ingen http-, shell-, fs- eller opener-rettigheter; utover core:* bare mappevalg, sjekk/installer og omstart", () => {
    const dir = join(desktop, "src-tauri", "capabilities");
    for (const f of readdirSync(dir).filter((n) => n.endsWith(".json"))) {
      const cap = JSON.parse(readFileSync(join(dir, f), "utf8")) as { permissions: (string | { identifier: string })[] };
      const ids = cap.permissions.map((p) => (typeof p === "string" ? p : p.identifier));
      const rest = ids.filter((i) => !EXTRA_PERMISSIONS.includes(i));
      expect(rest.filter((i) => /^(http|shell|fs|opener|updater|websocket|process|os|dialog)\b|:allow-(fetch|open|spawn|execute)/.test(i)), f).toEqual([]);
      expect(rest.every((i) => i.startsWith("core:")), `${f}: ${ids.join(",")}`).toBe(true);
    }
  });

  test("oppdatering i tauri.conf.json: ett endepunkt (GitHub-release «beta»), fast offentlig nøkkel, kun HTTPS", () => {
    const conf2 = JSON.parse(readFileSync(join(desktop, "src-tauri", "tauri.conf.json"), "utf8")) as {
      plugins?: { updater?: { endpoints?: string[]; pubkey?: string; dangerousInsecureTransportProtocol?: boolean } };
    };
    const u = conf2.plugins?.updater;
    expect(u?.endpoints).toEqual(["https://github.com/robertkolakowski-ai/fjell/releases/download/beta/latest.json"]);
    expect(u?.pubkey).toBe("dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEJCN0NFNjU2REU1OTE1NTcKUldSWEZWbmVWdVo4dS9IWi85QUU1bEo1c2FLVnhSWGRZaUFQSm1nYWxwNmxkNk9TRkVjdTF5WXQK");
    expect(u?.dangerousInsecureTransportProtocol).toBeUndefined();
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

  test("tauri-plugin-updater (beta-kanalen, M9) finnes bare i skallets Cargo.toml (desktop/src-tauri), ingen andre crates", () => {
    const manifests = [...walk(join(repo, "core", "crates"), (p) => p.endsWith("Cargo.toml")), join(desktop, "src-tauri", "shell", "Cargo.toml")].filter(existsSync);
    expect(manifests.filter((m) => /tauri-plugin-updater/.test(readFileSync(m, "utf8"))).map((m) => relative(repo, m))).toEqual([]);
    expect(readFileSync(join(desktop, "src-tauri", "Cargo.toml"), "utf8")).toMatch(/^\s*tauri-plugin-updater\s*=/m);
  });

  test("egne crates har ingen direkte nettverksavhengighet i Cargo.toml", () => {
    const manifests = [...walk(join(repo, "core", "crates"), (p) => p.endsWith("Cargo.toml")), join(desktop, "src-tauri", "Cargo.toml"), join(desktop, "src-tauri", "shell", "Cargo.toml")].filter(existsSync);
    const hits = manifests.filter((m) => /^\s*(reqwest|hyper|ureq|curl|isahc|surf|attohttpc|tokio|tungstenite|tauri-plugin-(http|shell|fs|opener|websocket))\b/m.test(readFileSync(m, "utf8"))).map((m) => relative(repo, m));
    expect(hits).toEqual([]);
  });
});
