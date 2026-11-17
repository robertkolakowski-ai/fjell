// Sikkerhetskopi av indeksen (docs/v1/BACKUP.md) mot MockBridge: personvern- og flyttenotat, ta kopi, gjenoppretting
// med avkrysning («Avbryt» er først), og at pseudo-lokalet ikke kuttes. Mocken har ingen ekte filer; kjernen og
// skallet er testet i Rust.
import { expect, test } from "@playwright/test";
import { open } from "./helpers";

test("backupseksjonen forklarer hva kopien inneholder og hva som skjer ved flytting til ny PC", async ({ page }) => {
  await open(page, "#/settings");
  const group = page.getByRole("group", { name: "Sikkerhetskopi av indeksen" });
  await expect(group).toContainText("aldri bildene dine");
  await expect(group).toContainText("mappestier, personnavn og ansikts-embeddings");
  await expect(group).toContainText("ikke kryptert");
  await expect(group).toContainText("forlater den aldri");
  await expect(group).toContainText("ny PC");
});

test("gjenoppretting: Avbryt først, Gjenopprett krever avkrysning, og resultatet kan angres", async ({ page }) => {
  await open(page, "#/settings");
  await page.getByRole("button", { name: /Gjenopprett kopien: Egen kopi/ }).click();
  const dialog = page.getByRole("dialog");
  const buttons = dialog.getByRole("button");
  await expect(buttons.first()).toHaveText("Avbryt");
  const confirm = dialog.getByRole("button", { name: "Gjenopprett", exact: true });
  await expect(confirm).toBeDisabled();
  await dialog.getByRole("checkbox").check();
  await confirm.click();
  await expect(page.locator(".banner.ok")).toContainText("Indeksen er gjenopprettet.");
  // Gjenopprettingen laget en kopi av «før», og den kan angres.
  await expect(page.getByRole("list", { name: "Kopier av indeksen" }).getByText("Før gjenoppretting")).toBeVisible();
  await page.getByRole("button", { name: "Angre" }).click();
  await expect(page.locator(".banner.ok")).toHaveCount(0);
});

test("Ta kopi nå kan brukes med tastaturet og gir en ny kopi", async ({ page }) => {
  await open(page, "#/settings");
  const list = page.getByRole("list", { name: "Kopier av indeksen" });
  const before = await list.getByRole("listitem").count();
  const btn = page.getByRole("button", { name: "Ta kopi nå" });
  await btn.focus();
  await page.keyboard.press("Enter");
  await expect(list.getByRole("listitem")).toHaveCount(before + 1);
});

test("pseudo-lokalet kutter ikke backupseksjonen", async ({ page }) => {
  await open(page, "#/settings", "pseudo");
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBeLessThanOrEqual(0);
});
