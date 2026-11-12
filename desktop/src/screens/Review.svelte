<!-- Gjennomgang (UX-3, PLAN-UX-I18N 3.3): rutenett (uskarpe, skjermbilder) eller grupperader (duplikater), fast bunnlinje
     «Flytt N til papirkurv (X)», bekreftelsesark ved ≥ 50, Angre etter commit. Tilstand i reviewState.ts.
     Taster: Mellomrom/Enter velger (knapper), Shift+klikk område, Ctrl+A velg alle, Delete = bunnlinjeknappen; alle bare med fokus i listen. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { groupCards } from "../bridge/groups";
  import { KIND_CODES, type BridgeError, type Card, type KindCode } from "../bridge/types";
  import { useBridge } from "../context";
  import { Badge, BottomBar, Button, PhotoTile } from "../lib";
  import { fmt, reasonLabel, tr, type TFunction } from "../lib/i18n";
  import { formatHash } from "../router";
  import {
    initialReview, isProtected, PAGE_SIZE, reasonSnake, reduceReview, summarize, type ReviewEvent, type ReviewState,
  } from "./reviewState";

  interface Props {
    bridge?: CoreBridge;
    kind?: KindCode | null;
  }
  let { bridge, kind }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);

  const catKey = {
    exact_dup: "home.cat.exact_dup", visual_dup: "home.cat.visual_dup",
    blurry: "home.cat.blurry", screenshot: "home.cat.screenshot",
  } as const satisfies Record<KindCode, string>;

  let view: ReviewState = $state(initialReview);
  let days = $state(30);
  let dialog: HTMLDialogElement | undefined = $state();
  let sentinel: HTMLElement | undefined = $state();
  let shift = false;
  let gone = false;
  let observer: IntersectionObserver | undefined;

  const grouped = $derived(kind === "exact_dup" || kind === "visual_dup");
  const free = $derived(view.items.filter((c) => !isProtected(c)));
  const prot = $derived(view.items.filter(isProtected));
  const rows = $derived(grouped ? groupCards(free) : []);
  const sum = $derived(summarize(view.items, view.selected));
  const busy = $derived(view.phase === "committing");

  function send(e: ReviewEvent): void {
    view = reduceReview(view, e);
  }
  async function loadPage(append: boolean): Promise<void> {
    if (!kind) return;
    try {
      const page = await b.listCards(kind, PAGE_SIZE, append ? view.next : null);
      if (!gone) send({ type: "loaded", cards: page.cards, next: page.next, append });
    } catch (e) {
      if (!gone) send({ type: "failed", error: e as BridgeError });
    }
  }
  function reload(): void {
    send({ type: "reload" });
    void loadPage(false);
  }
  async function run(): Promise<void> {
    if (view.phase !== "committing") return;
    const ids = view.items.filter((c) => view.selected.has(c.id) && !isProtected(c)).map((c) => c.id);
    try {
      send({ type: "committed", batch: await b.decide(ids, "trash") });
    } catch (e) {
      send({ type: "commitFailed", error: e as BridgeError });
    }
  }
  function request(): void {
    send({ type: "requestCommit" });
    void run();
  }
  function confirm(): void {
    send({ type: "confirmCommit" });
    void run();
  }
  async function undo(): Promise<void> {
    try { await b.undo(); } catch { /* ingenting å angre: last på nytt uansett */ }
    reload();
  }

  // Dialogen styres av tilstand. Esc (cancel/close) går tilbake via onclose.
  $effect(() => {
    if (!dialog) return;
    if (view.confirming && !dialog.open) dialog.showModal();
    else if (!view.confirming && dialog.open) dialog.close();
  });
  // Infinite list: neste side når bunnen synes. Uten IntersectionObserver (test/gamle visninger) hentes resten med én gang.
  $effect(() => {
    observer?.disconnect();
    if (!sentinel || view.next === null || view.phase === "loading") return;
    if (typeof IntersectionObserver === "undefined") { void loadPage(true); return; }
    observer = new IntersectionObserver((en) => { if (en.some((x) => x.isIntersecting)) void loadPage(true); });
    observer.observe(sentinel);
  });
  onMount(() => {
    b.getState().then((s) => { if (!gone) days = s.trashDays; }).catch(() => undefined);
    void loadPage(false);
  });
  onDestroy(() => { gone = true; observer?.disconnect(); });

  function keydown(e: KeyboardEvent): void {
    const el = e.target as HTMLElement;
    if (el.closest("input, textarea, select") || e.altKey || e.metaKey) return;
    if (e.ctrlKey && e.key.toLowerCase() === "a") { e.preventDefault(); send({ type: "selectAll" }); }
    else if (e.key === "Delete" && !e.ctrlKey && !e.shiftKey) { e.preventDefault(); request(); }
  }
  const order = $derived(free.map((c) => c.id));
  function pick(id: number): void {
    send(shift ? { type: "range", id, order } : { type: "toggle", id });
    shift = false;
  }

  function badges(c: Card, _t: TFunction): { label: string; tone?: "warn" }[] {
    const code = reasonSnake(c.reason);
    return [{ label: reasonLabel(code, code === "blurry" ? { pct: fmt.percent(c.confidence) } : {}) }];
  }
  const nameOf = (c: Card, i: number, total: number, t: TFunction): string =>
    t("review.photo.name", { n: i + 1, total, size: fmt.bytes(c.bytes) }) + ` ${badges(c, t)[0].label}.`;
</script>

<section class="review">
  <h1 tabindex="-1">{$tr("review.title")}{#if kind}<span class="sub"> · {$tr(catKey[kind])}</span>{/if}</h1>
  <p><a href={formatHash({ name: "home" })}>{$tr("review.back")}</a></p>

  {#if !kind}
    <ul class="cats">
      {#each KIND_CODES as k (k)}
        <li><a class="cat" href={formatHash({ name: "review", kind: k })}>{$tr(catKey[k])}</a></li>
      {/each}
    </ul>
  {:else if view.phase === "loading"}
    <p role="status">{$tr("common.loading")}</p>
  {:else if view.phase === "error" && view.items.length === 0}
    <p role="alert">{$tr("review.error.body")}</p>
    <Button onclick={reload}>{$tr("common.retry")}</Button>
  {:else if view.phase === "empty" || (view.items.length === 0 && view.phase !== "committed")}
    <h2>{$tr("review.empty.title")}</h2>
    <p>{$tr("review.empty.body")}</p>
  {:else}
    {#if view.error}<p role="alert">{$tr("review.error.body")}</p>{/if}
    {#if grouped}<p class="muted">{$tr("group.exact.note")}</p>{/if}

    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="list" role="group" aria-label={$tr("review.title")} onkeydown={keydown} onclickcapture={(e) => (shift = e.shiftKey)}>
      {#if grouped}
        <ul class="rows">
          {#each rows as g (g.id)}
            {@const ids = g.cards.map((c) => c.id)}
            {@const on = ids.every((id) => view.selected.has(id))}
            <li class="row" class:on>
              <button type="button" class="pick" aria-pressed={on} disabled={busy}
                aria-label="{$tr('review.group.row', { count: g.cards.length })}, {fmt.bytes(g.bytes)}"
                onclick={() => send({ type: "toggleMany", ids })}>
                <span class="mark" aria-hidden="true">{on ? "✓" : ""}</span>
                <span class="best"><Badge label={$tr("review.group.best")} /></span>
                <span>{$tr("review.group.row", { count: g.cards.length })}</span>
                <span class="muted">{fmt.bytes(g.bytes)}</span>
              </button>
              <a href={formatHash({ name: "group", id: g.id })}>{$tr("group.title")}</a>
            </li>
          {/each}
        </ul>
      {:else}
        <ul class="grid">
          {#each free as c, i (c.id)}
            <li>
              <PhotoTile label={nameOf(c, i, free.length, $tr) + (view.selected.has(c.id) ? ` ${$tr("review.select.state.on")}.` : "")}
                selected={view.selected.has(c.id)} disabled={busy} badges={badges(c, $tr)} onToggle={() => pick(c.id)} />
            </li>
          {/each}
        </ul>
      {/if}

      {#if prot.length > 0}
        <h2>{$tr("review.protected.title")}</h2>
        <p class="muted">{$tr("review.protected.note")}</p>
        <ul class="grid">
          {#each prot as c, i (c.id)}
            <li><PhotoTile label={nameOf(c, i, prot.length, $tr)} disabled badges={badges(c, $tr)} /></li>
          {/each}
        </ul>
      {/if}
    </div>

    {#if view.next !== null}<p bind:this={sentinel} class="muted" role="status">{$tr("review.more")}</p>{/if}

    <BottomBar
      label={sum.count === 0 ? $tr("review.bar.none") : $tr("review.bar.trash", { count: sum.count, size: fmt.bytes(sum.bytes) })}
      announce={$tr("a11y.live.selected", { count: sum.count, size: fmt.bytes(sum.bytes) })}
      disabled={sum.count === 0 || busy} onclick={request} ariaLabel={$tr("review.title")}>
      {#snippet secondary()}
        {#if busy}
          <span role="status">{$tr("review.committing")}</span>
        {:else if view.phase === "committed"}
          <span role="status">{$tr("review.committed", { count: view.moved })}</span>
          <Button onclick={undo}>{$tr("common.undo")}</Button>
        {/if}
        <Button onclick={() => send({ type: "selectAll" })} ariaKeyshortcuts="Control+A">{$tr("review.select.all")}</Button>
        <Button onclick={() => send({ type: "clear" })}>{$tr("review.select.none")}</Button>
      {/snippet}
    </BottomBar>
  {/if}

  <dialog bind:this={dialog} aria-labelledby="rv-ct" onclose={() => send({ type: "cancelConfirm" })}>
    <h2 id="rv-ct">{$tr("review.confirm.title")}</h2>
    <p>{$tr("review.confirm.preview", { count: sum.count, size: fmt.bytes(sum.bytes) })}</p>
    <p>{$tr("review.confirm.body", { days })}</p>
    <label class="ack">
      <input type="checkbox" checked={view.ack} onchange={(e) => send({ type: "setAck", value: e.currentTarget.checked })} />
      <span>{$tr("review.confirm.ack")}</span>
    </label>
    <div class="actions">
      <Button onclick={() => send({ type: "cancelConfirm" })}>{$tr("common.cancel")}</Button>
      <Button variant="primary" disabled={!view.ack} onclick={confirm}>{$tr("review.confirm.go")}</Button>
    </div>
  </dialog>
</section>

<style>
  .review { max-inline-size: 60rem; }
  h1 { margin-block: 0 var(--space-3); font-size: var(--fs-xl); line-height: var(--lh-tight); }
  h1:focus { outline: none; }
  h2 { font-size: var(--fs-lg); margin-block: var(--space-5) var(--space-2); }
  .sub { font-weight: var(--fw-regular, 400); }
  .muted { color: var(--c-text-muted); }
  ul { list-style: none; margin: 0; padding: 0; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr)); gap: var(--space-3); }
  .rows { display: grid; gap: var(--space-3); }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-4); padding: var(--space-2) var(--space-3); border: var(--border-width) solid var(--c-border); border-radius: var(--radius-md); background: var(--c-surface); }
  .row.on { border: 3px solid var(--c-selected); padding: calc(var(--space-2) - 2px) calc(var(--space-3) - 2px); }
  .pick { display: flex; flex: 1 1 14rem; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-3); min-block-size: var(--target-min); padding: 0; border: 0; background: none; color: var(--c-text); font: inherit; text-align: start; cursor: pointer; }
  .mark { display: inline-grid; place-items: center; inline-size: 1.5rem; block-size: 1.5rem; border: 2px solid var(--c-border-strong); border-radius: var(--radius-sm); font-weight: var(--fw-semibold); }
  .cats { display: grid; gap: var(--space-3); }
  .cat { display: flex; align-items: center; min-block-size: var(--target-min); padding: var(--space-3) var(--space-4); color: var(--c-text); border: var(--border-width) solid var(--c-border); border-radius: var(--radius-lg); background: var(--c-surface); text-decoration: none; }
  dialog { max-inline-size: min(32rem, calc(100vw - 2rem)); padding: var(--space-5); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-lg); background: var(--c-surface); color: var(--c-text); }
  dialog::backdrop { background: rgb(0 0 0 / 0.5); }
  dialog h2 { margin-block-start: 0; }
  .ack { display: flex; align-items: flex-start; gap: var(--space-2); margin-block: var(--space-3); }
  .ack input { inline-size: 1.25rem; block-size: 1.25rem; flex: none; margin-block-start: 0.125rem; }
  .actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: var(--space-3); }
</style>
