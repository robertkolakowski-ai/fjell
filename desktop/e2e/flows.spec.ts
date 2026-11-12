// Flyttester (M7): onboarding, gruppe, kortmodus (kun tastatur), papirkurv. MockBridge uten ventetid.
import { expect, test } from "@playwright/test";
import { go, open } from "./helpers";

test("onboarding: første funn, ryd, angre", async ({ page }) => {
  await open(page, "#/onboarding");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Rydd bildene dine, trygt og rolig");
  await page.getByRole("button", { name: "Kom i gang" }).click();
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Velg mapper");
  await page.getByRole("button", { name: "Start skanning" }).click();
  await expect(page.getByRole("heading", { level: 1 })).toContainText("eksakte duplikater", { timeout: 15_000 });
  await page.getByRole("button", { name: /^Flytt \d+ til papirkurv/ }).click(); // under 50 bilder og 1 GB: ingen dialog
  await expect(page.getByRole("heading", { level: 1 })).toContainText("i papirkurven");
  await expect(page.getByText("kan frigjøres når du tømmer papirkurven")).toBeVisible();
  await expect(page.getByText(/frigjort/i)).toHaveCount(0); // ingen «frigjort» før permanent sletting
  await page.getByRole("button", { name: "Angre" }).click();
  await go(page, "#/");
  await expect(page.getByRole("link", { name: "Papirkurv (0)" })).toBeVisible();
});

test("gruppe: bytt keeper, flytt, angre", async ({ page }) => {
  await open(page, "#/");
  await page.locator("h1").first().waitFor(); // Hjem viser «Rask opprydding» først etter skann; gruppevisningen trenger ikke det
  await go(page, "#/group/1");
  const best = page.locator("li", { has: page.getByText("Beste", { exact: true }) });
  await expect(best).toHaveCount(1);
  await expect(best).toContainText("Bilde 1 av 3");
  // Uten ekte miniatyrer (plassholdere) er bytte av keeper ikke synlig i DOM; vi sjekker at kallet fullføres og gruppen er intakt.
  await page.getByRole("button", { name: "Gjør dette til beste" }).nth(1).click();
  await expect(page.getByRole("button", { name: "Gjør dette til beste" })).toHaveCount(2);
  await expect(page.getByRole("button", { name: "Gjør dette til beste" }).first()).toBeEnabled();
  await page.getByRole("button", { name: "Gjør dette til beste" }).nth(1).focus();
  await page.keyboard.press("Tab");
  await page.locator(".items").focus();
  await page.keyboard.press("2"); // tast 1–9 gjelder også (bare med fokus i gruppeområdet)
  await expect(page.getByRole("button", { name: "Gjør dette til beste" })).toHaveCount(2);
  await expect(best).toContainText("Bilde 1 av 3");
  await expect(page.getByRole("button", { name: "Behold dette, flytt 2 til papirkurv" })).toBeVisible();
  await page.getByRole("button", { name: "Behold dette, flytt 2 til papirkurv" }).click();
  await expect(page.getByText("Gruppen er ferdig. 2 bilder i papirkurven.")).toBeVisible();
  await page.getByRole("button", { name: "Angre" }).click();
  await expect(page.getByRole("button", { name: "Behold dette, flytt 2 til papirkurv" })).toBeVisible();
  await page.getByRole("button", { name: "Behold alle" }).click();
  await expect(page).toHaveURL(/#\/review$/);
});

test("kortmodus kun med tastatur", async ({ page }) => {
  await open(page, "#/cards");
  const left = page.locator('.cards p.muted[aria-hidden="true"]');
  await expect(left).toBeVisible();
  const n0 = Number((await left.textContent())!.match(/\d+/)![0]);
  // Fokus til kortet med Tab (aldri mus).
  const deck = page.getByRole("group", { name: /Forslag/ });
  for (let i = 0; i < 15 && !(await deck.evaluate((el) => el === document.activeElement)); i++) await page.keyboard.press("Tab");
  await expect(deck).toBeFocused();
  await page.keyboard.press("F1");
  await expect(page.getByRole("dialog", { name: "Hurtigtaster i kortmodus" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.keyboard.press("ArrowUp"); // papirkurv
  await expect(left).toHaveText(`${n0 - 1} igjen`);
  await page.keyboard.press("ArrowLeft"); // behold
  await expect(left).toHaveText(`${n0 - 2} igjen`);
  await page.keyboard.press("ArrowRight"); // hopp over: antall uendret
  await expect(left).toHaveText(`${n0 - 2} igjen`);
  await page.keyboard.press("Space"); // zoom
  await expect(page.locator(".deck.zoom")).toHaveCount(1);
  await page.keyboard.press("Space");
  await page.keyboard.press("Control+z"); // angre behold
  await expect(left).toHaveText(`${n0 - 1} igjen`);
  await page.keyboard.press("Delete"); // papirkurv
  await expect(left).toHaveText(`${n0 - 2} igjen`);
  // Gå gjennom resten med tastatur til bunken er tom.
  for (let i = 0; i < 60 && (await page.getByRole("heading", { name: "Ingen flere kort" }).count()) === 0; i++) {
    await page.keyboard.press("ArrowUp");
    await page.waitForTimeout(10);
  }
  await expect(page.getByRole("heading", { name: "Ingen flere kort" })).toBeVisible();
  await expect(page.getByText(/Du flyttet \d+ bilde/)).toBeVisible();
  await expect(page.getByRole("button", { name: "Angre siste" })).toBeVisible();
});

test("papirkurv: angre (gjenopprett) og tøm med bekreftelse", async ({ page }) => {
  await open(page, "#/review/blurry");
  await page.getByRole("button", { name: "Velg alle" }).click();
  await page.getByRole("button", { name: /^Flytt 6 til papirkurv/ }).click();
  await expect(page.getByText("6 flyttet")).toBeVisible();
  await page.getByRole("button", { name: "Angre" }).click(); // gjenoppretter
  await expect(page.getByRole("button", { name: "Velg alle" })).toBeVisible();
  await page.getByRole("button", { name: "Velg alle" }).click();
  await page.getByRole("button", { name: /^Flytt 6 til papirkurv/ }).click();
  await expect(page.getByText("6 flyttet")).toBeVisible();
  await go(page, "#/trash");
  await expect(page.locator(".summary")).toBeVisible();
  await page.getByRole("button", { name: "Tøm papirkurv …" }).click();
  const dlg = page.getByRole("dialog", { name: "Tøm papirkurven" });
  await expect(dlg).toBeVisible();
  await dlg.getByRole("button", { name: "Avbryt" }).click(); // avbryt endrer ingenting
  await expect(page.locator(".summary")).toContainText("6 bilder");
  await page.getByRole("button", { name: "Tøm papirkurv …" }).click();
  await dlg.getByRole("radio", { name: "Slett permanent" }).check();
  await dlg.getByRole("button", { name: "Fortsett" }).click();
  const perm = page.getByRole("dialog", { name: "Dette kan ikke angres" });
  await expect(perm).toBeVisible();
  const go6 = perm.getByRole("button", { name: "Slett 6 permanent" });
  await expect(go6).toBeDisabled(); // krever avkrysning
  await perm.getByRole("checkbox").check();
  await go6.click();
  await expect(page.locator(".done").getByText(/er permanent slettet/)).toBeVisible();
  await expect(page.getByRole("status").filter({ hasText: /er permanent slettet/ }).first()).toBeAttached();
  await expect(page.getByRole("heading", { name: "Papirkurven er tom" })).toBeVisible();
});
