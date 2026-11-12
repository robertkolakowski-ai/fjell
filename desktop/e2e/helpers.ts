import type { Page } from "@playwright/test";

export type Lang = "nb" | "en" | "pseudo";

/** Åpner appen på en hash-rute med gitt språk, mock uten ventetid. `extra` er flere dev-parametre, f.eks. `&mockUpdate=available`. */
export async function open(page: Page, hash = "#/", lang: Lang = "nb", delay = 0, extra = ""): Promise<void> {
  await page.addInitScript((l) => {
    try { localStorage.setItem("fjell.language", l); } catch { /* ignorer */ }
  }, lang);
  await page.goto(`/?mockDelay=${delay}${extra}${hash}`);
  await page.locator("h1").first().waitFor();
}

/** Naviger via hash i samme økt (beholder MockBridge-tilstand). */
export async function go(page: Page, hash: string): Promise<void> {
  await page.evaluate((h) => { location.hash = h; }, hash);
  await page.locator("h1").first().waitFor();
}

const samples: number[] = [];
export const perfSamples = samples;

/** Måler tid fra handling (utført i siden) til DOM viser forventet resultat + neste frame. ms. */
export async function timed(page: Page, action: string, done: string, label = ""): Promise<number> {
  const ms = await page.evaluate(
    ({ action, done }) =>
      new Promise<number>((resolve, reject) => {
        const act = new Function(action) as () => void;
        const ok = new Function(`return (${done});`) as () => boolean;
        const fail = setTimeout(() => { mo.disconnect(); reject(new Error("tidsavbrudd: " + done)); }, 3000);
        const finish = (t0: number) => {
          clearTimeout(fail); mo.disconnect();
          requestAnimationFrame(() => resolve(performance.now() - t0));
        };
        let t0 = 0;
        const mo = new MutationObserver(() => { if (ok()) finish(t0); });
        mo.observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
        t0 = performance.now();
        act();
        if (ok()) finish(t0);
      }),
    { action, done },
  );
  samples.push(ms);
  void label;
  return ms;
}

/** Kjører rask skann via onboarding slik at Hjem/Review/Cards har data (MockBridge beholder tilstand i økten). */
export async function seedScan(page: Page): Promise<void> {
  await go(page, "#/onboarding");
  await page.getByRole("button", { name: /Kom i gang|Get started/ }).click();
  await page.getByRole("button", { name: /Start skanning|Start scan/ }).click();
  await page.getByRole("button", { name: /^(Flytt|Move) \d+/ }).waitFor({ timeout: 15_000 });
}
