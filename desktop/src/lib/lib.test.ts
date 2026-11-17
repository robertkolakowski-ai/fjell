import { test, expect } from "vitest";
import { debounce } from "./debounce.ts";
import { nb } from "./i18n/strings/nb.ts";
import { en } from "./i18n/strings/en.ts";

test("debounce: bare siste kall slipper gjennom, cancel stopper", async () => {
  const seen: string[] = [];
  const d = debounce((s: string) => seen.push(s), 20);
  d("a"); d("b"); d("c");
  await new Promise((r) => setTimeout(r, 50));
  expect(seen).toEqual(["c"]);
  d("x"); d.cancel();
  await new Promise((r) => setTimeout(r, 50));
  expect(seen).toEqual(["c"]);
});

// Personvern: wipe-teksten og backup-notatet skal være ærlige om at kopier av indeksen ikke omfattes (BEHANDLINGSOVERSIKT, funn 1).
test("wipe og backup.privacy sier at kopier ikke omfattes, har embeddings og er ukrypterte", () => {
  for (const key of ["settings.faces.wipe.desc", "people.index.wipe.confirm"] as const) {
    expect(nb[key]).toMatch(/Sikkerhetskopier/);
    expect(en[key]).toMatch(/Backups/);
  }
  expect(nb["backup.privacy"]).toMatch(/embeddings/);
  expect(nb["backup.privacy"]).toMatch(/ikke kryptert/);
  expect(en["backup.privacy"]).toMatch(/embeddings/);
  expect(en["backup.privacy"]).toMatch(/not encrypted/);
});
