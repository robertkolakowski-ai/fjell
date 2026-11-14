<!-- Sammenlign serie (`#/series/:id`, underside av Gjennomgang): medlemmene side om side med rangering, poeng og begrunnelse.
     Behold-kandidaten er forhåndsmerket (hake + tekst + ramme, aldri bare farge); alt annet er forhåndsmerket for papirkurv, unntatt
     beskyttede bilder. «Gjør dette til beste» (knapp eller tast 1–9 i bildeområdet), felles zoom (knapper, + − 0, Ctrl + hjul),
     «Behold alle» og «Flytt N til papirkurv» er hver ett angrebart valg. Appen foreslår; brukeren bestemmer. Tilstand i seriesState.ts. -->
<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import type { BridgeError } from "../bridge/types";
  import { useBridge } from "../context";
  import Thumb from "../lib/components/Thumb.svelte";
  import { Badge, Banner, BottomBar, Button, CloudWarning, Icon, LiveRegion, PageHeader, ProtectToggle } from "../lib";
  import { fmt, reasonLabel, tr, type TFunction } from "../lib/i18n";
  import { formatHash } from "../router";
  import { cloudFromError, loadCloud } from "./cloud";
  import CardExplain from "./CardExplain.svelte";
  import { isProtectedAck, PROTECTED_ACK } from "./protectAck";
  import { loadRuleNames, type RuleNames } from "./reason";
  import { reasonSnake } from "./reviewState";
  import {
    allCardIds, initialSeries, keeperForKey, loadSeries, markedBytes, moveIds, reduceSeries, seriesItems, zoomKey,
    type SeriesEvent, type SeriesItem, type SeriesState,
  } from "./seriesState";

  interface Props {
    bridge?: CoreBridge;
    id: number;
  }
  let { bridge, id }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);

  let view: SeriesState = $state(initialSeries);
  let gone = false;
  let cloud: string | null = $state(null);
  let ruleNames: RuleNames = $state(new Map());
  let live = $state("");
  let liveSeq = 0;
  /** Siste beskyttelsesendring (til melding med Angre). */
  let protectNote: { asset: number; n: number; next: boolean } | null = $state(null);
  let list: HTMLElement | undefined = $state();
  let done: HTMLElement | undefined = $state();
  let dialog: HTMLDialogElement | undefined = $state();
  let syncing = false;

  const items = $derived(seriesItems(view));
  const ids = $derived(moveIds(view));
  const busy = $derived(view.phase === "working");
  const eyesUnavailable = $derived(
    view.compare !== null && items.length > 0 && items[0].reason === "KeeperBestOverall" && !items.some((i) => i.closedEyes),
  );

  function send(e: SeriesEvent): void { view = reduceSeries(view, e); }
  const say = (m: string): void => { liveSeq++; live = m + (liveSeq % 2 ? "" : " "); };

  async function load(): Promise<void> {
    try {
      const r = await loadSeries(b, id);
      if (!gone) send({ type: "loaded", compare: r.compare, cards: r.cards });
    } catch (e) {
      if (!gone) send({ type: "failed", error: e as BridgeError });
    }
  }
  function retry(): void {
    send({ type: "reload" });
    void load();
  }
  /** Ny henting etter en endring (ny behold-kandidat, endret beskyttelse). */
  async function refresh(): Promise<void> {
    const r = await loadSeries(b, id);
    if (!gone) send({ type: "refreshed", compare: r.compare, cards: r.cards });
  }
  // Ny serie-id i ruten gjenbruker komponenten: last den nye serien.
  // svelte-ignore state_referenced_locally
  let shownId = id;
  $effect(() => {
    const n = id;
    untrack(() => { if (n !== shownId) { shownId = n; protectNote = null; retry(); } });
  });
  onMount(() => {
    void load();
    void loadCloud(b).then((c) => { if (!gone) cloud = c; });
    void loadRuleNames(b).then((n) => { if (!gone) ruleNames = n; });
  });
  onDestroy(() => { gone = true; });

  async function makeBest(asset: number): Promise<void> {
    if (view.phase !== "ready") return;
    send({ type: "start" });
    try {
      await b.setKeeper(id, asset);
      await refresh();
      const n = seriesItems(view).find((i) => i.asset === asset)?.n ?? 1;
      say($tr("series.live.keeper", { n }));
      await tick();
      list?.querySelector<HTMLElement>(`[data-name="${asset}"]`)?.focus(); // knappen som ble trykket er borte: fokus til bildets navn
    } catch (e) {
      send({ type: "actionFailed", error: e as BridgeError });
    }
  }
  async function moveRest(confirmed: string[] = []): Promise<void> {
    if (view.phase !== "ready" || ids.length === 0 || cloud) return;
    const n = ids.length;
    send({ type: "start" });
    try {
      await b.decide(ids, "trash", confirmed);
      send({ type: "committed", done: "trash", moved: n });
    } catch (e) {
      const err = e as BridgeError;
      if (isProtectedAck(err)) { send({ type: "askProtected" }); return; }
      cloud = cloudFromError(err) ?? cloud;
      send({ type: "actionFailed", error: err });
    }
  }
  async function keepAll(): Promise<void> {
    if (view.phase !== "ready") return;
    const all = allCardIds(view);
    if (all.length === 0) return;
    send({ type: "start" });
    try {
      await b.decide(all, "keep");
      send({ type: "committed", done: "keep", moved: all.length });
    } catch (e) {
      send({ type: "actionFailed", error: e as BridgeError });
    }
  }
  async function setProtect(it: SeriesItem, next: boolean): Promise<void> {
    if (view.phase !== "ready") return;
    send({ type: "start" });
    try {
      await b.protect({ scope: "asset", target: String(it.asset) }, next);
      await refresh();
      protectNote = { asset: it.asset, n: it.n, next };
      say($tr(next ? "series.protect.changed.on" : "series.protect.changed.off", { n: it.n }));
    } catch (e) {
      send({ type: "actionFailed", error: e as BridgeError });
    }
  }
  async function undoProtect(): Promise<void> {
    const p = protectNote;
    if (!p || view.phase !== "ready") return;
    protectNote = null;
    send({ type: "start" });
    try {
      await b.protect({ scope: "asset", target: String(p.asset) }, !p.next);
      await refresh();
    } catch (e) {
      send({ type: "actionFailed", error: e as BridgeError });
    }
  }
  async function undo(): Promise<void> {
    try { await b.undo(); } catch { /* ingenting å angre */ }
    protectNote = null;
    retry();
  }

  // Dialogen styres av tilstand (Esc går tilbake via onclose).
  $effect(() => {
    if (!dialog) return;
    if (view.askProtected && !dialog.open) dialog.showModal();
    else if (!view.askProtected && dialog.open) dialog.close();
  });
  // Valgene er borte etter en ferdig handling: fokus til meldingen (ellers mistes fokus).
  $effect(() => {
    if (view.phase === "committed") void tick().then(() => done?.focus());
  });

  function zoomTo(e: "zoomIn" | "zoomOut" | "zoomReset"): void {
    send({ type: e });
    say($tr("series.zoom.level", { pct: fmt.percent(view.zoom) }));
  }
  function keydown(e: KeyboardEvent): void {
    if (e.ctrlKey || e.altKey || e.metaKey || (e.target as HTMLElement).closest("input, textarea, select")) return;
    const z = zoomKey(e);
    if (z) { e.preventDefault(); zoomTo(z); return; }
    const asset = keeperForKey(items, e.key);
    if (asset !== null) { e.preventDefault(); void makeBest(asset); }
  }
  /** Ctrl + musehjul zoomer alle bildene (knappene gjør det samme). Må være ikke-passiv for å stoppe nettleserzoom. */
  function ctrlWheel(node: HTMLElement): { destroy(): void } {
    const h = (e: WheelEvent): void => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      zoomTo(e.deltaY < 0 ? "zoomIn" : "zoomOut");
    };
    node.addEventListener("wheel", h, { passive: false });
    return { destroy: () => node.removeEventListener("wheel", h) };
  }
  /** Rulling i ett bilde følger i alle (relativ posisjon), så samme utsnitt vises overalt. */
  function sync(e: Event): void {
    if (syncing) return;
    const src = e.currentTarget as HTMLElement;
    const fx = src.scrollWidth > src.clientWidth ? src.scrollLeft / (src.scrollWidth - src.clientWidth) : 0;
    const fy = src.scrollHeight > src.clientHeight ? src.scrollTop / (src.scrollHeight - src.clientHeight) : 0;
    syncing = true;
    for (const el of list?.querySelectorAll<HTMLElement>(".img") ?? []) {
      if (el === src) continue;
      el.scrollLeft = fx * (el.scrollWidth - el.clientWidth);
      el.scrollTop = fy * (el.scrollHeight - el.clientHeight);
    }
    requestAnimationFrame(() => { syncing = false; });
  }

  const reasonOf = (it: SeriesItem, _t: TFunction): string =>
    reasonLabel(reasonSnake(it.reason), {
      pct: fmt.percent(it.score), index: it.detail.kind === "position" ? it.detail.index : undefined,
      of: it.detail.kind === "position" ? it.detail.of : undefined,
    });
  const nameOf = (it: SeriesItem, total: number, t: TFunction): string => t("series.member.name", { n: it.n, total });
</script>

<section class="series" data-series={id}>
  <PageHeader title={$tr("series.compare.title")} back={{ href: formatHash({ name: "review", kind: "series" }), label: $tr("review.back") }} />
  {#if cloud}<CloudWarning provider={cloud} />{/if}
  <LiveRegion message={live} />

  {#if view.phase === "loading"}
    <p role="status">{$tr("common.loading")}</p>
  {:else if view.phase === "error" && !view.compare}
    <Banner tone="danger" role="alert">
      <p>{view.error?.code === "unknown_series" ? $tr("error.unknown_series") : $tr("series.error.body")}</p>
      {#snippet actions()}<Button onclick={retry}>{$tr("common.retry")}</Button>{/snippet}
    </Banner>
  {:else if view.phase === "committed"}
    <div class="done" bind:this={done} tabindex="-1">
      <Banner tone="ok" role="status">
        <p>{view.done === "trash" ? $tr("series.committed", { count: view.moved }) : $tr("series.kept")}</p>
        {#snippet actions()}<Button onclick={undo}>{$tr("common.undo")}</Button>{/snippet}
      </Banner>
    </div>
  {:else if view.compare}
    {#if view.error}<Banner tone="danger" role="alert"><p>{$tr(cloudFromError(view.error) ? "cloud.error.blocked" : "series.error.body")}</p></Banner>{/if}
    {#if protectNote}
      <Banner tone="info" role="status">
        <p>{$tr(protectNote.next ? "series.protect.changed.on" : "series.protect.changed.off", { n: protectNote.n })}</p>
        {#snippet actions()}<Button size="sm" disabled={busy} onclick={undoProtect}>{$tr("common.undo")}</Button>{/snippet}
      </Banner>
    {/if}

    <p class="muted lead">{$tr("series.compare.lead")}<br />{$tr("group.keys.hint")}</p>
    {#if eyesUnavailable}<p class="muted lead" data-testid="eyes-unavailable">{$tr("series.eyes.unavailable")}</p>{/if}

    <div class="zoombar" role="group" aria-label={$tr("common.zoom")}>
      <Button size="sm" onclick={() => zoomTo("zoomOut")} disabled={view.zoom <= 1} ariaKeyshortcuts="-">{$tr("group.zoom.out")}</Button>
      <Button size="sm" onclick={() => zoomTo("zoomIn")} disabled={view.zoom >= 4} ariaKeyshortcuts="+">{$tr("group.zoom.in")}</Button>
      <Button size="sm" onclick={() => zoomTo("zoomReset")} disabled={view.zoom === 1} ariaKeyshortcuts="0">{$tr("series.zoom.reset")}</Button>
      <span class="zlevel" aria-hidden="true">{$tr("series.zoom.level", { pct: fmt.percent(view.zoom) })}</span>
    </div>
    <p class="muted hint">{$tr("series.zoom.hint")}</p>

    <div class="cq">
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <ul class="items" aria-label={$tr("series.compare.title")} bind:this={list} onkeydown={keydown} use:ctrlWheel>
        {#each items as it (it.asset)}
          <li class="item" class:best={it.best} data-asset={it.asset}>
            <div class="media">
              <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
              <div class="img" style:--z={view.zoom} role="group" tabindex={view.zoom > 1 ? 0 : undefined}
                aria-label={view.zoom > 1 ? nameOf(it, items.length, $tr) : undefined} onscroll={sync}>
                <div class="zoomer"><Thumb id={it.asset} fit="contain" alt={nameOf(it, items.length, $tr)} /></div>
              </div>
              <span class="num" aria-hidden="true">{it.n}</span>
              {#if it.best}
                <span class="tick" aria-hidden="true"><Icon name="check" /></span>
              {/if}
            </div>
            <p class="name" tabindex="-1" data-name={it.asset}>{nameOf(it, items.length, $tr)}</p>
            <div class="meta">
              {#if it.best}<Badge label={$tr("series.keeper.badge")} />{/if}
              {#if it.closedEyes}<Badge tone="warn" label={$tr("reason.closed_eyes")} />{/if}
              {#if it.protected}<Badge tone="warn" label={$tr("group.protected")} />{/if}
            </div>
            <dl>
              <dt>{$tr("series.info.rank")}</dt><dd data-info="rank">{fmt.number(it.rank)}</dd>
              <dt>{$tr("series.info.score")}</dt><dd data-info="score">{fmt.number(it.score)}</dd>
              <dt>{$tr("series.info.reason")}</dt><dd data-info="reason" class="reason">{reasonOf(it, $tr)}</dd>
              <dt>{$tr("group.info.size")}</dt><dd>{fmt.bytes(it.bytes)}</dd>
            </dl>
            <CardExplain reason={it.reason} detail={it.detail} {ruleNames} />
            {#if it.markable}
              <label class="mark">
                <input type="checkbox" checked={it.marked} disabled={busy} onchange={() => send({ type: "toggleMark", asset: it.asset })} />
                <span>{$tr("series.mark.move")}</span>
              </label>
            {/if}
            <div class="acts">
              {#if !it.best}
                <Button size="sm" disabled={busy} ariaKeyshortcuts={it.n <= 9 ? String(it.n) : undefined} onclick={() => makeBest(it.asset)}>{$tr("group.make.best")}</Button>
              {/if}
              <ProtectToggle scope={{ scope: "asset", target: String(it.asset) }} protected={it.protected} disabled={busy}
                label={$tr("series.protect.label", { n: it.n })} onchange={(next) => setProtect(it, next)} />
            </div>
          </li>
        {/each}
      </ul>
    </div>

    <BottomBar ariaLabel={$tr("series.compare.title")}
      announce={$tr("series.live.marked", { count: ids.length }) + ` (${fmt.bytes(markedBytes(view))})`}
      label={$tr("series.move.primary", { count: ids.length })}
      disabled={busy || (!cloud && ids.length === 0)} blocked={!!cloud} describedby="cloud-why" onclick={() => moveRest()}>
      {#snippet secondary()}
        <Button disabled={busy} onclick={keepAll}>{$tr("group.keep.all")}</Button>
      {/snippet}
    </BottomBar>
  {/if}

  <dialog bind:this={dialog} aria-labelledby="sr-at" onclose={() => send({ type: "dismissAsk" })}>
    <h2 id="sr-at">{$tr("series.ack.title")}</h2>
    <p>{$tr("series.ack.body")}</p>
    <div class="actions">
      <Button onclick={() => send({ type: "dismissAsk" })}>{$tr("series.ack.cancel")}</Button>
      <Button variant="danger" onclick={() => moveRest([PROTECTED_ACK])}>{$tr("series.ack.go")}</Button>
    </div>
  </dialog>
</section>

<style>
  .series { max-inline-size: var(--w-wide); }
  .cq { container-type: inline-size; }
  .muted { color: var(--c-text-muted); }
  .lead { margin: 0 0 var(--space-3); font-size: var(--fs-sm); max-inline-size: 62ch; }
  .hint { margin: var(--space-1) 0 var(--space-4); font-size: var(--fs-sm); max-inline-size: 62ch; }
  .done:focus { outline: none; }
  :global(.series .banner p) { margin: 0; }
  ul { list-style: none; margin: 0; padding: 0; }
  .zoombar { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2); }
  .zlevel { font-variant-numeric: var(--num); font-size: var(--fs-sm); }

  .items { display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--space-4); }
  .item { display: flex; flex-direction: column; gap: var(--space-2); min-inline-size: 0; padding: var(--space-3); border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-lg); background: var(--c-surface); box-shadow: var(--shadow-card); }
  .item.best { border: 3px solid var(--c-ok); padding: calc(var(--space-3) - 2px); background: var(--c-ok-bg); }
  .media { position: relative; }
  .img { position: relative; overflow: auto; aspect-ratio: 4 / 3; border-radius: var(--radius-md); background: var(--c-surface-sunken); }
  .zoomer { position: relative; inline-size: calc(var(--z, 1) * 100%); block-size: calc(var(--z, 1) * 100%); }
  .num { position: absolute; inset-block-start: var(--space-2); inset-inline-start: var(--space-2); min-inline-size: 1.75rem; padding: 0 var(--space-2); text-align: center; background: var(--c-surface); color: var(--c-text); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-pill); font-weight: var(--fw-semibold); font-variant-numeric: var(--num); pointer-events: none; }
  .tick { position: absolute; inset-block-start: var(--space-2); inset-inline-end: var(--space-2); display: grid; place-items: center; inline-size: 1.75rem; block-size: 1.75rem; border-radius: var(--radius-pill); background: var(--c-ok); color: var(--c-surface); pointer-events: none; }
  .tick :global(svg) { inline-size: 1.125rem; block-size: 1.125rem; }
  .name { margin: 0; font-weight: var(--fw-semibold); }
  .name:focus-visible { outline: 2px solid var(--c-focus, currentColor); outline-offset: 2px; }
  .meta { display: flex; flex-wrap: wrap; gap: var(--space-1); }
  .meta:empty { display: none; }
  dl { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: var(--space-1) var(--space-3); margin: 0; padding-block-start: var(--space-2); border-block-start: var(--border-width) solid var(--c-border-subtle); font-size: var(--fs-sm); }
  dt { color: var(--c-text-muted); }
  dd { margin: 0; font-weight: var(--fw-semibold); font-variant-numeric: var(--num); overflow-wrap: anywhere; }
  .mark { display: flex; align-items: flex-start; gap: var(--space-2); min-block-size: var(--target-min); }
  .mark input { inline-size: 1.25rem; block-size: 1.25rem; flex: none; margin-block-start: 0.125rem; }
  .acts { display: flex; flex-wrap: wrap; gap: var(--space-2); margin-block-start: auto; }
  dialog { max-inline-size: min(32rem, calc(100vw - 2rem)); padding: var(--space-5); border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-lg); background: var(--c-surface); color: var(--c-text); box-shadow: var(--shadow-pop); }
  dialog::backdrop { background: var(--c-scrim); }
  dialog h2 { margin-block-start: 0; font-size: var(--fs-lg); }
  .actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: var(--space-3); }

  /* Containerspørringer (rem følger rot-skriftstørrelsen): holder ved 200 % og stor skrift. */
  @container (min-width: 36rem) { .items { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  @container (min-width: 60rem) { .items { grid-template-columns: repeat(auto-fit, minmax(min(100%, var(--tile-min)), 1fr)); } }
  @media (forced-colors: active) {
    .item.best { border-color: Highlight; }
    .tick { background: Highlight; color: HighlightText; }
  }
</style>
