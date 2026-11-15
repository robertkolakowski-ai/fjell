// Konformitet: MockBridge mot samme scenarier som den ekte kjernen (`conformance_v1.json`, kjørt i Rust av
// `core/.../fjell_v1_tests/conformance.rs`). Begge sider må gi lik utgang for beskyttede bilder (`ack_required`,
// `SCOPE_FLAG`), serie- og regelkandidater, wipe av ansiktsindeksen og feilkodene. Kjernen er sannheten:
// avviker mocken, rettes mocken (ikke fixturen), og begge sider endres i samme commit.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { MockBridge } from "./mock-bridge";
import { SCOPE_FLAG, assetPath } from "./mock-v1";
import type { BridgeError, Card, KindCode } from "./types";
import { KIND_CODES } from "./types";
import type { Condition, Scope } from "./types-v1";

const FIXTURE = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "core", "crates", "fjell-core", "src", "api", "fjell_v1_tests", "conformance_v1.json");

type Expect = "ok" | { code: string; detail?: string };
interface Step { op: string; [k: string]: unknown }
interface Template { name: string; kinds?: string[]; scopes: string[]; steps: Step[] }
interface ErrorCase { name: string; op: string; expect: Expect; [k: string]: unknown }
interface Fixture { version: number; scope_flag: number; kinds: string[]; templates: Template[]; wipe: Step[]; errors: ErrorCase[] }

const fx = JSON.parse(readFileSync(FIXTURE, "utf8")) as Fixture;
const MISSING = 4_000_000_000;
const LIBRARY = "C:\\Bilder";

const mock = () => new MockBridge({ now: () => Date.UTC(2026, 10, 14) });
const all = async (b: MockBridge, kind: KindCode): Promise<Card[]> => (await b.listCards(kind, 200)).cards;

type Out = { ok: true } | { ok: false; code: string; detail: string };
async function run(p: Promise<unknown>): Promise<Out> {
  try {
    await p;
    return { ok: true };
  } catch (e) {
    const { code, detail } = e as BridgeError;
    return { ok: false, code, detail: detail ?? "" };
  }
}

function check(expected: Expect, got: Out, ctx: string): void {
  if (expected === "ok") {
    expect(got, ctx).toEqual({ ok: true });
    return;
  }
  expect(got.ok, `${ctx}: forventet feil ${expected.code}`).toBe(false);
  if (got.ok) return;
  expect(got.code, ctx).toBe(expected.code);
  if (expected.detail !== undefined) expect(got.detail, ctx).toBe(expected.detail);
}

class Env {
  readonly b = mock();
  cards: Card[] = [];
  snap = 0;
  personAssets = 0;
  facesTotal = 0;
  constructor(readonly kind: KindCode, readonly scope: string) {}
  async init(): Promise<this> {
    this.cards = await all(this.b, this.kind);
    return this;
  }
  scopeOf(card: number): Scope {
    const a = this.cards[card].asset;
    if (this.scope === "asset") return { scope: "asset", target: String(a) };
    const p = assetPath(LIBRARY, a);
    const dir = p.slice(0, p.lastIndexOf("\\"));
    // Samme varianter som Rust-siden: hele mappenavnet, avkortet (beskytter ikke), annen skrivemåte (beskytter ikke).
    const target = this.scope === "folder_truncated" ? dir.slice(0, -1) : this.scope === "folder_upper" ? dir.toUpperCase() : dir;
    return { scope: "folder", target };
  }
  async listed(card: number): Promise<Card | undefined> {
    return (await all(this.b, this.kind)).find((c) => c.id === this.cards[card].id);
  }
  async ruleHits(card: number): Promise<number> {
    const d = this.cards[card].detail;
    if (d.kind !== "rule") throw new Error("ikke et regelkort");
    return (await this.b.listRules()).find((r) => r.id === d.id)!.hits;
  }
}

const n = (v: unknown): number => v as number;
const s = (v: unknown): string => v as string;

async function steps(env: Env, list: Step[], ctx: string): Promise<void> {
  for (const [i, st] of list.entries()) {
    const c = `${ctx} / steg ${i} (${st.op})`;
    const b = env.b;
    switch (st.op) {
      case "protect":
        await b.protect(env.scopeOf(n(st.card)), st.on as boolean);
        break;
      case "expect_flag": {
        const card = await env.listed(n(st.card));
        expect(card, `${c}: kortet vises ikke`).toBeDefined();
        expect((card!.protectFlags & SCOPE_FLAG) !== 0, c).toBe(st.flagged);
        break;
      }
      case "decide": {
        const ids = (st.cards as number[]).map((k) => env.cards[k].id);
        check(st.expect as Expect, await run(b.decide(ids, s(st.decision) as "trash" | "keep", st.confirmed as string[])), c);
        break;
      }
      case "expect_pending":
        expect((await env.listed(n(st.card))) !== undefined, c).toBe(st.pending);
        break;
      case "expect_member_flags": {
        const card = env.cards[n(st.protected_card)];
        const cmp = await b.compareSeries(card.group!);
        for (const m of cmp.members) expect((m.protectFlags & SCOPE_FLAG) !== 0, `${c}: ${m.asset}`).toBe(m.asset === card.asset);
        break;
      }
      case "protect_keeper": {
        const cmp = await b.compareSeries(env.cards[0].group!);
        await b.protect({ scope: "asset", target: String(cmp.keeper) }, true);
        break;
      }
      case "expect_keeper_flag": {
        const cmp = await b.compareSeries(env.cards[0].group!);
        const m = cmp.members.find((x) => x.isKeeper)!;
        expect((m.protectFlags & SCOPE_FLAG) !== 0, c).toBe(st.flagged);
        break;
      }
      case "snapshot_rule_hits":
        env.snap = await env.ruleHits(n(st.card));
        break;
      case "expect_rule_hits_delta":
        expect((await env.ruleHits(n(st.card))) - env.snap, c).toBe(st.delta);
        break;
      case "expect_assets_unique_across_kinds": {
        const seen = new Set<number>();
        for (const k of KIND_CODES) {
          for (const card of await all(b, k)) {
            expect(seen.has(card.asset), `${c}: asset ${card.asset} i flere kort`).toBe(false);
            seen.add(card.asset);
          }
        }
        break;
      }
      case "enable_faces":
        await b.setFaceIndex(true);
        break;
      case "expect_people_at_least":
        expect((await b.listPeople()).length, c).toBeGreaterThanOrEqual(n(st.n));
        break;
      case "protect_person": {
        const people = await b.listPeople();
        const p = people[n(st.index)];
        env.personAssets = p.assetCount;
        env.facesTotal = people.reduce((x, y) => x + y.faceCount, 0);
        check(st.expect as Expect, await run(b.protectPerson(p.id, true)), c);
        break;
      }
      case "expect_scope_counts": {
        const prot = await b.listProtections();
        const count = (person: boolean): number => prot.filter((e) => (e.scope.scope === "person") === person).length;
        expect(count(true), c).toBe(n(st.person));
        if (st.asset_equals_person_assets !== undefined) expect(count(false), c).toBe(env.personAssets);
        break;
      }
      case "wipe":
        expect(st.expect).toBe("faces_total");
        expect(await b.wipeFaceIndex(), c).toBe(env.facesTotal);
        break;
      case "expect_people_count":
        expect((await b.listPeople()).length, c).toBe(n(st.n));
        break;
      default:
        throw new Error(`${c}: ukjent operasjon`);
    }
  }
}

async function errorCase(e: ErrorCase): Promise<void> {
  const ctx = `feil: ${e.name}`;
  const b = mock();
  const name = (): string => (e.name_len !== undefined ? "x".repeat(n(e.name_len)) : s(e.rule_name));
  const conds = (): Condition[] => e.conditions as Condition[];
  let got: Out;
  switch (e.op) {
    case "protect_target": {
      const t = s(e.target);
      const target = t === "missing" ? String(MISSING) : t;
      got = await run(b.protect({ scope: s(e.scope) as Scope["scope"], target }, true));
      break;
    }
    case "decide_missing": got = await run(b.decide([MISSING], "trash")); break;
    case "compare_missing_series": got = await run(b.compareSeries(MISSING)); break;
    case "save_rule": got = await run(b.saveRule({ id: null, name: name(), conditions: conds() })); break;
    case "preview_rule": {
      got = await run(b.previewRule(conds()));
      if (got.ok && e.expect_hits !== undefined) expect((await b.previewRule(conds())).hits, ctx).toBe(e.expect_hits);
      break;
    }
    case "toggle_missing_rule": got = await run(b.toggleRule(MISSING, true)); break;
    case "delete_missing_rule": got = await run(b.deleteRule(MISSING)); break;
    case "restore_missing_rule": got = await run(b.restoreRule(MISSING)); break;
    case "name_missing_person": got = await run(b.namePerson(MISSING, "x")); break;
    default: throw new Error(`${ctx}: ukjent operasjon`);
  }
  check(e.expect, got, ctx);
}

describe("MockBridge er lik kjernen (conformance_v1.json)", () => {
  it("fixturen er den samme versjonen og bruker samme flagg som mocken", () => {
    expect(fx.version).toBe(1);
    expect(fx.scope_flag).toBe(SCOPE_FLAG);
  });

  for (const t of fx.templates) {
    const kinds = fx.kinds.filter((k) => !t.kinds || t.kinds.includes(k));
    for (const kind of kinds) {
      for (const scope of t.scopes) {
        it(`${t.name} [${kind}, ${scope}]`, async () => {
          const env = await new Env(kind as KindCode, scope).init();
          expect(env.cards.length).toBeGreaterThan(0);
          await steps(env, t.steps, t.name);
        });
      }
    }
  }

  it("wipe: personbeskyttelse blir bildebeskyttelse, ingen mister vernet", async () => {
    const env = await new Env("exact_dup", "asset").init();
    await steps(env, fx.wipe, "wipe");
  });

  for (const e of fx.errors) it(`feilkode: ${e.name}`, () => errorCase(e));
});
