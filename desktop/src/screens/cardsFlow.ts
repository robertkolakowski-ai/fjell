// Kortmodus: flyt mot CoreBridge. Reduceren oppdaterer UI umiddelbart (optimistisk); kjernekall går i seriell kø
// slik at «angre» alltid treffer riktig batch. Brukes av Cards.svelte og testes mot MockBridge.
import type { CoreBridge } from "../bridge/core-bridge";
import { KIND_CODES, type BridgeError, type Card, type KindCode } from "../bridge/types";
import { PAGE_SIZE } from "./reviewState";
import { initialCards, reduceCards, type CardAction, type CardsEvent, type CardsState } from "./cardsState";

/** Henter alle sider (maks `cap` kort) for én kategori, eller alle fire når `kind` er null. */
export async function loadDeck(b: CoreBridge, kind: KindCode | null, cap = 1000): Promise<Card[]> {
  const out: Card[] = [];
  for (const k of kind ? [kind] : KIND_CODES) {
    let cursor: number | null = null;
    do {
      const page = await b.listCards(k, PAGE_SIZE, cursor);
      out.push(...page.cards);
      cursor = page.next;
    } while (cursor !== null && out.length < cap);
  }
  return out;
}

export class CardsSession {
  state: CardsState = initialCards;
  private chain: Promise<unknown> = Promise.resolve();
  private gone = false;

  constructor(
    private readonly b: CoreBridge,
    private readonly kind: KindCode | null,
    private readonly onChange: (s: CardsState) => void = () => undefined,
  ) {}

  send(e: CardsEvent): void {
    if (this.gone) return;
    this.state = reduceCards(this.state, e);
    this.onChange(this.state);
  }
  dispose(): void { this.gone = true; }

  async load(): Promise<void> {
    this.send({ type: "reload" });
    try {
      this.send({ type: "loaded", cards: await loadDeck(this.b, this.kind) });
    } catch (e) {
      this.send({ type: "failed", error: e as BridgeError });
    }
  }

  /** Behold/papirkurv/hopp over. Returnerer når kjernekallet er ferdig (UI er allerede oppdatert). */
  act(action: CardAction): Promise<void> {
    const top = this.state.phase === "card" ? this.state.queue[0] : undefined;
    this.send({ type: "act", action });
    if (!top || action === "skip") return this.chain.then(() => undefined);
    return this.enqueue(async () => {
      try {
        await this.b.decide([top.id], action);
      } catch (e) {
        this.send({ type: "rollback", card: top, error: e as BridgeError });
      }
    });
  }

  undo(): Promise<void> {
    if (this.state.history.length === 0) return this.chain.then(() => undefined);
    this.send({ type: "undo" });
    return this.enqueue(async () => {
      try {
        await this.b.undo();
      } catch {
        await this.load(); // kjernen og UI er ute av takt: last bunken på nytt fra kjernen
      }
    });
  }

  private enqueue(fn: () => Promise<void>): Promise<void> {
    const p = this.chain.then(fn);
    this.chain = p.catch(() => undefined);
    return p;
  }
}
