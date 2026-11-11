import { describe, expect, it } from "vitest";
import { initialState, reduce, type Event, type UiState } from "./state";

const run = (evs: Event[], from: UiState = initialState) => evs.reduce(reduce, from);
const toReview: Event[] = [
  { type: "scanStart" },
  { type: "scanProgress", phase: "analyzing", done: 5, total: 10 },
  { type: "scanProgress", phase: "done", done: 10, total: 10 },
];

describe("tilstandsmaskin", () => {
  it("skann går til gjennomgang ved done", () => {
    expect(run(toReview.slice(0, 2)).step).toBe("scanning");
    expect(run(toReview).step).toBe("review");
  });
  it("tom markering gir ikke bekreftelse", () => {
    expect(run([...toReview, { type: "requestConfirm" }]).step).toBe("review");
  });
  it("hele løpet: velg → bekreft → papirkurv → angre", () => {
    let s = run([...toReview, { type: "toggle", id: 1 }, { type: "toggle", id: 2 }, { type: "requestConfirm" }]);
    expect(s.step).toBe("confirm");
    s = run([{ type: "confirm" }, { type: "trashed", batch: 4 }], s);
    expect(s).toMatchObject({ step: "trashed", undoBatch: 4, lastCount: 2 });
    expect(s.selected.size).toBe(0);
    s = run([{ type: "undo" }, { type: "undone" }], s);
    expect(s).toMatchObject({ step: "review", undoBatch: null });
  });
  it("avbryt bekreftelse beholder markering", () => {
    const s = run([...toReview, { type: "toggle", id: 1 }, { type: "requestConfirm" }, { type: "cancelConfirm" }]);
    expect(s.step).toBe("review");
    expect(s.selected.has(1)).toBe(true);
  });
  it("toggle to ganger fjerner", () => {
    expect(run([...toReview, { type: "toggle", id: 1 }, { type: "toggle", id: 1 }]).selected.size).toBe(0);
  });
  it("feil i papirkurv går tilbake til gjennomgang med markering og feil", () => {
    const s = run([
      ...toReview, { type: "toggle", id: 1 }, { type: "requestConfirm" }, { type: "confirm" },
      { type: "trashFailed", error: { code: "core_error", detail: "x" } },
    ]);
    expect(s).toMatchObject({ step: "review", error: { code: "core_error" } });
    expect(s.selected.size).toBe(1);
  });
  it("angrefeil beholder angremuligheten", () => {
    const s = run([
      ...toReview, { type: "toggle", id: 1 }, { type: "requestConfirm" }, { type: "confirm" },
      { type: "trashed", batch: 1 }, { type: "undo" },
      { type: "undoFailed", error: { code: "not_undoable", detail: "" } },
    ]);
    expect(s).toMatchObject({ step: "trashed", undoBatch: 1 });
  });
  it("ugyldige overganger ignoreres", () => {
    expect(run([{ type: "confirm" }]).step).toBe("idle");
    expect(run([{ type: "trashed", batch: 1 }]).step).toBe("idle");
    expect(run([{ type: "undo" }]).step).toBe("idle");
    expect(run([{ type: "toggle", id: 1 }]).selected.size).toBe(0);
  });
  it("skannfeil gir idle med feil", () => {
    expect(run([{ type: "scanStart" }, { type: "scanFailed", error: { code: "core_error", detail: "" } }]))
      .toMatchObject({ step: "idle", error: { code: "core_error" } });
  });
  it("er ren: gir ikke mutasjon av forrige tilstand", () => {
    const a = run(toReview);
    const sel = a.selected;
    reduce(a, { type: "toggle", id: 5 });
    expect(a.selected).toBe(sel);
    expect(sel.size).toBe(0);
  });
});
