// Ytelse: handling til DOM-oppdatering (+ neste frame), p95 < 100 ms. Mock uten ventetid: måler UI-lagets kostnad, ikke Tauri/kjerne.
import { mkdirSync, writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { go, open, seedScan, timed } from "./helpers";

const p = (xs: number[], q: number): number => {
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.ceil(q * s.length) - 1)];
};
const r1 = (n: number) => Math.round(n * 10) / 10;

test("handling til DOM p95 < 100 ms", async ({ page }) => {
  test.setTimeout(120_000);
  await page.setViewportSize({ width: 1280, height: 800 });
  const groups: Record<string, number[]> = {};
  const add = (k: string, ms: number) => (groups[k] ??= []).push(ms);

  await open(page, "#/");
  await seedScan(page);
  await go(page, "#/review/blurry");
  await page.getByRole("button", { name: "Velg alle" }).waitFor();

  // Velg og fjern enkeltbilder
  for (let r = 0; r < 4; r++) {
    for (let i = 0; i < 6; i++) {
      add("velg bilde", await timed(page, `document.querySelectorAll('.tile')[${i}].click()`, `document.querySelectorAll('.tile')[${i}].getAttribute('aria-pressed')==='true'`));
    }
    for (let i = 0; i < 6; i++) {
      add("fjern valg", await timed(page, `document.querySelectorAll('.tile')[${i}].click()`, `document.querySelectorAll('.tile')[${i}].getAttribute('aria-pressed')==='false'`));
    }
  }
  const btn = (name: string) => `[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==='${name}')`;
  for (let r = 0; r < 8; r++) {
    add("velg alle", await timed(page, `${btn("Velg alle")}.click()`, `document.querySelectorAll('.tile[aria-pressed=true]').length===6`));
    add("fjern alle", await timed(page, `${btn("Fjern alle valg")}.click()`, `document.querySelectorAll('.tile[aria-pressed=true]').length===0`));
  }

  // Navigasjon mellom skjermer
  const nav = (n: string) => `[...document.querySelectorAll('nav a')].find(a=>a.textContent.trim()==='${n}')`;
  for (let r = 0; r < 6; r++) {
    add("naviger til Hjem", await timed(page, `${nav("Hjem")}.click()`, `document.querySelector('h1')?.textContent==='Hjem' && !!document.querySelector('.home button')`));
    add("naviger til Papirkurv", await timed(page, `${nav("Papirkurv")}.click()`, `document.querySelector('h1')?.textContent==='Papirkurv' && !!document.querySelector('.trash p.summary, .trash h2')`));
    add("naviger til Gjennomgang", await timed(page, `${nav("Gjennomgang")}.click()`, `document.querySelector('h1')?.textContent==='Gjennomgang' && !!document.querySelector('.cats')`));
  }

  // Kortmodus: tastetrykk til neste kort (teller endres)
  await go(page, "#/cards");
  await page.locator(".deck").waitFor();
  const counter = `document.querySelector('.cards p.muted[aria-hidden="true"]')`;
  const keys: [string, string][] = [["ArrowUp", "papirkurv"], ["ArrowLeft", "behold"]];
  for (let i = 0; i < 16; i++) {
    const [key, name] = keys[i % 2];
    const m = await timed(
      page,
      `window.__b=${counter}?.textContent; document.querySelector('.deck').dispatchEvent(new KeyboardEvent('keydown',{key:'${key}',bubbles:true,cancelable:true}))`,
      `!${counter} || ${counter}.textContent!==window.__b`,
    );
    add(`kort ${name} (tast)`, m);
  }

  // Bekreftelsesark og Tøm-ark
  await go(page, "#/review/screenshot");
  await page.getByRole("button", { name: "Velg alle" }).click();
  add("flytt valgte (commit)", await timed(page, `document.querySelector('.bar button.primary, .bar [class*=primary] button, .bar .primary button')?.click() ?? [...document.querySelectorAll('button')].find(b=>/^Flytt \\d+ til papirkurv/.test(b.textContent.trim())).click()`, `/\\d+ flyttet/.test(document.body.textContent)`));
  await go(page, "#/trash");
  await page.locator(".summary").waitFor();
  add("åpne Tøm-ark", await timed(page, `${btn("Tøm papirkurv …")}.click()`, `document.querySelector('dialog[open]')!==null`));

  const all = Object.values(groups).flat();
  const rows = Object.entries(groups).map(([k, v]) => ({ handling: k, n: v.length, p50: r1(p(v, 0.5)), p95: r1(p(v, 0.95)), maks: r1(Math.max(...v)) }));
  const total = { n: all.length, p50: r1(p(all, 0.5)), p95: r1(p(all, 0.95)), maks: r1(Math.max(...all)) };
  mkdirSync("e2e/.results", { recursive: true });
  writeFileSync("e2e/.results/perf.json", JSON.stringify({ rows, total }, null, 2));
  console.log("PERF " + JSON.stringify({ total, rows }));
  expect(total.p95).toBeLessThan(100);
  for (const r of rows) expect(r.p95, r.handling).toBeLessThan(100);
});
