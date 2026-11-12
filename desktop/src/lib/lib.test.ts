import { test, expect } from "vitest";
import { debounce } from "./debounce.ts";

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
