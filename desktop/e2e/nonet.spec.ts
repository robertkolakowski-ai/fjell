// «Ingen nettverkskall» ved kjøring: gå gjennom alle skjermer og feil hvis siden ber om noe utenfor appens egen opprinnelse.
// (Statisk bevis: src/lib/nonet.test.ts.) Vite dev-serveren på localhost er appens opprinnelse i testen.
import { expect, test } from "@playwright/test";
import { tour } from "./tour";

test("ingen forespørsler utenfor appens opprinnelse, og ingen fetch/XHR/WebSocket/beacon fra appkode", async ({ page, baseURL }) => {
  test.setTimeout(120_000);
  const origin = new URL(baseURL ?? "http://localhost:1420").origin;
  const outside: string[] = [];
  const isLocal = (u: string) => u.startsWith(origin) || u.startsWith("data:") || u.startsWith("blob:") || u.startsWith("about:") || u.startsWith("ws://localhost:1420");
  // Blokker alt utenfor opprinnelsen slik at en lekkasje både feiler testen og aldri forlater maskinen.
  await page.route((u) => !isLocal(u.toString()), (route) => { outside.push(route.request().url()); return route.abort(); });
  page.on("request", (r) => { if (!isLocal(r.url())) outside.push(r.url()); });
  await page.addInitScript(() => {
    const w = window as unknown as { __net: string[] };
    w.__net = [];
    const rec = (kind: string, u: unknown) => {
      const s = String(u);
      // Vites HMR-klient (bare dev) kobler til localhost; det er ikke appkode.
      if (/^wss?:\/\/localhost(:\d+)?\//.test(s)) return;
      w.__net.push(`${kind} ${s}`);
    };
    const f = window.fetch.bind(window);
    window.fetch = ((i: RequestInfo | URL, init?: RequestInit) => { rec("fetch", i instanceof Request ? i.url : i); return f(i, init); }) as typeof fetch;
    const open = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function (this: XMLHttpRequest, m: string, u: string | URL, ...rest: unknown[]) { rec("xhr", u); return (open as (...a: unknown[]) => void).call(this, m, u, ...rest); } as typeof open;
    const WS = window.WebSocket;
    window.WebSocket = class extends WS { constructor(u: string | URL, p?: string | string[]) { rec("websocket", u); super(u, p); } } as typeof WebSocket;
    navigator.sendBeacon = (u: string | URL) => { rec("beacon", u); return false; };
    const ES = window.EventSource;
    if (ES) window.EventSource = class extends ES { constructor(u: string | URL, c?: EventSourceInit) { rec("eventsource", u); super(u, c); } } as typeof EventSource;
  });
  let screens = 0;
  await tour(page, async () => { screens++; });
  expect(screens).toBeGreaterThan(10);
  const calls = await page.evaluate(() => (window as unknown as { __net: string[] }).__net);
  expect(calls, `nettverks-API brukt: ${calls.join(", ")}`).toEqual([]);
  expect(outside, `forespørsler utenfor opprinnelsen: ${outside.join(", ")}`).toEqual([]);
});

test("kontroll: testen oppdager et eksternt kall (ikke en tom test)", async ({ page, baseURL }) => {
  const origin = new URL(baseURL ?? "http://localhost:1420").origin;
  const outside: string[] = [];
  await page.route((u) => !u.toString().startsWith(origin), (route) => { outside.push(route.request().url()); return route.abort(); });
  await page.goto("/?mockDelay=0#/");
  await page.evaluate(() => { void fetch("https://example.invalid/ping").catch(() => undefined); });
  await expect.poll(() => outside).toEqual(["https://example.invalid/ping"]);
});
