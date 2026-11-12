<!-- Kortmodus (UX-4, PLAN-UX-I18N 3.5): ett kort + to bak, knapper alltid til stede, hurtigtaster (6.4), valgfri mus-dra.
     Tastene er bare aktive når kortområdet har fokus (Ctrl+Z og F1 gjelder hele skjermen). Skjermleser: handlingene er
     vanlige knapper med aria-keyshortcuts, resultatet annonseres i live-region, dra-overlegget er aria-hidden. -->
<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import type { Card, KindCode } from "../bridge/types";
  import { useBridge } from "../context";
  import { Badge, Button, CloudWarning, LiveRegion } from "../lib";
  import { fmt, reasonLabel, tr as tx } from "../lib/i18n";
  import { formatHash } from "../router";
  import { cloudFromError, loadCloud } from "./cloud";
  import { CardsSession } from "./cardsFlow";
  import {
    ariaKeys, CARD_KEYS, cardKey, dragIntent, initialCards, type CardAction, type CardsState,
  } from "./cardsState";
  import { reasonSnake } from "./reviewState";

  interface Props {
    bridge?: CoreBridge;
    kind?: KindCode | null;
  }
  let { bridge, kind = null }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  // svelte-ignore state_referenced_locally
  const session = new CardsSession(b, kind, (s) => (view = s));

  let view: CardsState = $state(initialCards);
  let helpOpen = $state(false);
  /** Skysynket bibliotek (leverandørkode): papirkurv er sperret synlig, med forklaring. */
  let cloud: string | null = $state(null);
  /** Papirkurv-tast/knapp/dra opp er trykket i sperret tilstand: forklaring vises og annonseres. */
  let blockedShown = $state(false);
  let blockedSeq = $state(0);
  let help: HTMLDialogElement | undefined = $state();
  let deck: HTMLElement | undefined = $state();
  let summary: HTMLElement | undefined = $state();
  interface Drag { x: number; y: number; t: number; dx: number; dy: number }
  let drag = $state<Drag | null>(null);

  const top = $derived(view.queue[0]);
  const behind = $derived(view.queue.slice(1, 3));
  const left = $derived(view.queue.length);
  const total = $derived(view.queue.length + view.history.length);
  const intent = $derived.by(() => {
    const d = drag;
    return d ? dragIntent(d.dx, d.dy, deck?.clientWidth ?? 1, deck?.clientHeight ?? 1) : null;
  });
  const hintAction = $derived.by(() => {
    const d = drag;
    if (!d) return null;
    return Math.abs(d.dx) >= Math.abs(d.dy) ? (d.dx < 0 ? "keep" : "skip") : d.dy < 0 ? "trash" : null;
  });
  // Lik melding to ganger på rad (to «Hoppet over» med samme antall igjen) må bli et nytt innhold: veksle et usynlig tegn.
  const live = $derived.by(() => {
    if (blockedShown) return $tx("cloud.key.blocked") + (blockedSeq % 2 ? " " : "");
    if (!view.live) return "";
    const k = view.live.kind;
    const text = k === "undone" ? $tx("a11y.live.undone") : $tx(`a11y.live.${k}`, { left });
    return text + (view.live.seq % 2 ? " " : "");
  });

  const act = (a: CardAction): void => {
    if (a === "trash" && !top) return;
    if (a === "trash" && cloud) { blockedShown = true; blockedSeq++; return; } // sperret: gjør ingenting, men forklarer
    blockedShown = false;
    void session.act(a);
  };
  const undo = (): void => { blockedShown = false; void session.undo(); };
  // Skallet kan nekte papirkurv selv om UI ikke visste at mappen er skysynket: sperr da også her.
  $effect(() => { cloud = cloudFromError(view.error) ?? cloud; });
  const label = (c: Card): string =>
    `${$tx("cards.photo.alt", { n: total - left + 1, total })}. ${fmt.bytes(c.bytes)}. ${reasonLabel(reasonSnake(c.reason), { pct: fmt.percent(c.confidence) })}.`;

  function onKey(e: KeyboardEvent, scoped: boolean): void {
    const el = e.target as HTMLElement;
    const id = cardKey({
      key: e.key, ctrlKey: e.ctrlKey, shiftKey: e.shiftKey, altKey: e.altKey, metaKey: e.metaKey,
      inTextField: !!el.closest("input, textarea, select"), onButton: !!el.closest("button"),
    });
    if (!id) return;
    // Piler, Delete og Mellomrom bare med fokus i kortområdet; Ctrl+Z og F1 overalt.
    if (id !== "undo" && id !== "help" && !scoped) return;
    e.preventDefault();
    if (id === "keep" || id === "trash" || id === "skip") act(id);
    else if (id === "zoom") session.send({ type: "toggleZoom" });
    else if (id === "undo") undo();
    else helpOpen = true;
  }
  const windowKey = (e: KeyboardEvent): void => {
    if (deck?.contains(e.target as Node)) return; // håndteres av kortområdet
    if (helpOpen) return;
    onKey(e, false);
  };

  // Dra med mus/penn/touch er valgfritt; knapper og taster gjør det samme.
  function down(e: PointerEvent): void {
    if ((e.target as HTMLElement).closest("button") || e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    drag = { x: e.clientX, y: e.clientY, t: e.timeStamp, dx: 0, dy: 0 };
  }
  function move(e: PointerEvent): void {
    if (drag) drag = { ...drag, dx: e.clientX - drag.x, dy: e.clientY - drag.y };
  }
  function up(e: PointerEvent): void {
    if (!drag) return;
    const speed = Math.hypot(drag.dx, drag.dy) / Math.max(1, e.timeStamp - drag.t);
    const a = dragIntent(drag.dx, drag.dy, deck?.clientWidth ?? 1, deck?.clientHeight ?? 1, speed);
    drag = null;
    if (a) act(a);
  }

  // Hjelpedialogen styres av tilstand (Esc går tilbake via onclose).
  $effect(() => {
    if (!help) return;
    if (helpOpen && !help.open) help.showModal();
    else if (!helpOpen && help.open) help.close();
  });
  // Tom bunke: knappene forsvinner, så fokus går til oppsummeringen (ellers mistes fokus).
  $effect(() => {
    if (view.phase === "deckEmpty") void tick().then(() => summary?.focus());
  });
  onMount(() => {
    void session.load();
    void loadCloud(b).then((c) => { if (c) cloud = c; });
  });
  onDestroy(() => session.dispose());

  const keyName = (id: string): string => (id === "zoom" ? $tx("cards.key.space") : (CARD_KEYS.find((k) => k.id === id)?.label ?? id));
  const keyAction = (id: string): string =>
    ({ keep: $tx("cards.keep"), trash: $tx("cards.trash"), skip: $tx("cards.skip"), zoom: $tx("common.zoom"),
       undo: $tx("common.undo"), help: $tx("common.shortcuts") })[id] ?? id;
</script>

<svelte:window onkeydown={windowKey} />

<section class="cards">
  <h1 tabindex="-1">{$tx("cards.title")}</h1>
  <p><a href={formatHash({ name: "home" })}>{$tx("common.back")}</a></p>
  {#if cloud}<CloudWarning provider={cloud} />{/if}
  <LiveRegion message={live} />

  {#if view.phase === "loading"}
    <p role="status">{$tx("cards.loading")}</p>
  {:else if view.phase === "error"}
    <p role="alert">{$tx("cards.error.body")}</p>
    <Button onclick={() => void session.load()}>{$tx("common.retry")}</Button>
  {:else if view.phase === "deckEmpty"}
    <div class="end" bind:this={summary} tabindex="-1">
      <h2>{$tx("cards.empty.title")}</h2>
      <p>{$tx("cards.summary", { trashed: view.trashed, kept: fmt.number(view.kept) })}</p>
    </div>
    {#if view.history.length > 0}<Button onclick={undo} ariaKeyshortcuts={ariaKeys("undo")}>{$tx("cards.undo.last")}</Button>{/if}
  {:else if top}
    {#if view.error}<p role="alert">{$tx(cloudFromError(view.error) ? "cloud.error.blocked" : "review.error.body")}</p>{/if}
    {#if blockedShown}<p class="note">{$tx("cloud.key.blocked")}</p>{/if}
    <p class="muted" aria-hidden="true">{$tx("cards.remaining", { count: left })}</p>
    <p class="sr-only">{$tx("a11y.keys.focusmode")}</p>

    <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
    <div class="deck" class:zoom={view.zoom} role="group" tabindex="0" bind:this={deck}
      aria-label={label(top)} aria-keyshortcuts="ArrowLeft ArrowUp ArrowRight Delete Space"
      onkeydown={(e) => onKey(e, true)} onpointerdown={down} onpointermove={move} onpointerup={up}
      onpointercancel={() => (drag = null)} ondblclick={() => session.send({ type: "toggleZoom" })}>
      {#each [...behind].reverse() as c, i (c.id)}
        <div class="card back" style:--depth={behind.length - i} aria-hidden="true"></div>
      {/each}
      <div class="card front" style:transform={drag ? `translate(${drag.dx}px, ${drag.dy}px)` : undefined}>
        <span class="ph" aria-hidden="true"></span>
        <p class="meta">
          <Badge label={reasonLabel(reasonSnake(top.reason), { pct: fmt.percent(top.confidence) })} />
          <span class="muted">{fmt.bytes(top.bytes)}</span>
        </p>
        {#if hintAction}
          <span class="overlay" class:on={intent !== null} aria-hidden="true">
            {hintAction === "keep" ? "✓ " + $tx("cards.overlay.keep") : hintAction === "trash" ? "✕ " + $tx("cards.overlay.trash") : "→ " + $tx("cards.overlay.skip")}
          </span>
        {/if}
      </div>
    </div>
    <p class="muted hint">{$tx("cards.drag.hint")}</p>

    <div class="actions">
      <Button onclick={() => act("keep")} ariaKeyshortcuts={ariaKeys("keep")}>{$tx("cards.keep")}</Button>
      <Button onclick={() => act("trash")} ariaKeyshortcuts={ariaKeys("trash")} ariaDisabled={!!cloud} ariaDescribedby="cloud-why">{$tx("cards.trash")}</Button>
      <Button onclick={() => act("skip")} disabled={left < 2} ariaKeyshortcuts={ariaKeys("skip")}>{$tx("cards.skip")}</Button>
      <Button onclick={() => session.send({ type: "toggleZoom" })} ariaKeyshortcuts={ariaKeys("zoom")}>{$tx("common.zoom")}</Button>
      <Button onclick={undo} disabled={view.history.length === 0} ariaKeyshortcuts={ariaKeys("undo")}>{$tx("common.undo")}</Button>
      <Button onclick={() => (helpOpen = true)} ariaKeyshortcuts={ariaKeys("help")}>{$tx("common.shortcuts")}</Button>
    </div>
  {/if}

  <dialog bind:this={help} aria-labelledby="cd-ht" onclose={() => (helpOpen = false)}>
    <h2 id="cd-ht">{$tx("cards.help.title")}</h2>
    <table>
      <tbody>
        {#each CARD_KEYS as k (k.id)}
          <tr><th scope="row"><kbd>{keyName(k.id)}</kbd></th><td>{keyAction(k.id)}</td></tr>
        {/each}
      </tbody>
    </table>
    <p class="muted">{$tx("a11y.keys.focusmode")}</p>
    <div class="dlg-actions"><Button onclick={() => (helpOpen = false)}>{$tx("common.close")}</Button></div>
  </dialog>
</section>

<style>
  .cards { max-inline-size: 40rem; }
  h1 { margin-block: 0 var(--space-3); font-size: var(--fs-xl); line-height: var(--lh-tight); }
  h1:focus, .end:focus { outline: none; }
  .muted { color: var(--c-text-muted); }
  .note { margin-block: 0 var(--space-3); padding: var(--space-2) var(--space-3); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-md); }
  .hint { font-size: var(--fs-sm, 0.875rem); }
  .deck { position: relative; display: grid; min-block-size: 20rem; margin-block: var(--space-3); touch-action: none; cursor: grab; border-radius: var(--radius-lg); }
  .deck.zoom { min-block-size: min(32rem, 70vh); }
  .card { grid-area: 1 / 1; border: var(--border-width) solid var(--c-border); border-radius: var(--radius-lg); background: var(--c-surface); box-shadow: var(--shadow-sm); }
  .back { transform: translateY(calc(var(--depth) * 0.5rem)) scale(calc(1 - var(--depth) * 0.03)); background: var(--c-surface-sunken); }
  .front { position: relative; display: flex; flex-direction: column; padding: var(--space-3); gap: var(--space-3); }
  .ph { flex: 1; min-block-size: 12rem; border-radius: var(--radius-md); background: var(--c-surface-sunken); }
  .meta { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-3); margin: 0; }
  .overlay { position: absolute; inset-block-start: var(--space-3); inset-inline-start: var(--space-3); padding: var(--space-2) var(--space-3); border: 3px dashed var(--c-border-strong); border-radius: var(--radius-md); background: var(--c-surface); color: var(--c-text); font-weight: var(--fw-semibold); }
  .overlay.on { border-style: solid; border-color: var(--c-selected); }
  .actions { display: flex; flex-wrap: wrap; gap: var(--space-3); }
  @media (prefers-reduced-motion: no-preference) { .card { transition: transform var(--dur-fast); } }
  dialog { max-inline-size: min(32rem, calc(100vw - 2rem)); padding: var(--space-5); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-lg); background: var(--c-surface); color: var(--c-text); }
  dialog::backdrop { background: var(--c-scrim); }
  dialog h2 { margin-block-start: 0; }
  table { border-collapse: collapse; inline-size: 100%; }
  th, td { padding: var(--space-2) var(--space-3); text-align: start; border-block-end: var(--border-width) solid var(--c-border); }
  kbd { font: inherit; font-weight: var(--fw-semibold); padding: 0 var(--space-2); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-sm); }
  .dlg-actions { display: flex; justify-content: flex-end; margin-block-start: var(--space-3); }
</style>
