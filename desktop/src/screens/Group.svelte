<!-- Gruppevisning (UX-3, PLAN-UX-I18N 3.4): like bilder side om side, «Beste» forhåndsmerket (ikon + tekst + ramme),
     «Behold dette, flytt N» (primær), «Behold alle», «Gjør dette til beste» (ett trykk eller tast 1–9 når gruppeområdet har fokus).
     Beskyttede bilder flyttes aldri. Tilstand i groupState.ts. -->
<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import type { BridgeError, Group as Grp } from "../bridge/types";
  import { useBridge } from "../context";
  import Thumb from "../lib/components/Thumb.svelte";
  import { Badge, BottomBar, Banner, Button, CloudWarning, Icon, PageHeader } from "../lib";
  import { fmt, tr } from "../lib/i18n";
  import { formatHash, navigate } from "../router";
  import { cloudFromError, loadCloud } from "./cloud";
  import {
    findGroup, groupItems, listGroups, initialGroup, keeperForKey, movableCards, reduceGroup, type GroupEvent, type GroupState,
  } from "./groupState";

  interface Props {
    bridge?: CoreBridge;
    id: number;
  }
  let { bridge, id }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);

  let view: GroupState = $state(initialGroup);
  let gone = false;
  /** Skysynket bibliotek (leverandørkode): papirkurv er sperret synlig, med forklaring. */
  let cloud: string | null = $state(null);

  const items = $derived(view.group ? groupItems(view.group) : []);
  const movable = $derived(view.group ? movableCards(view.group) : []);
  const busy = $derived(view.phase === "working");

  function send(e: GroupEvent): void {
    view = reduceGroup(view, e);
  }
  async function load(): Promise<void> {
    try {
      const g = await findGroup(b, id);
      if (gone) return;
      if (g) send({ type: "loaded", group: g });
      else send({ type: "failed", error: { code: "unknown_card", detail: "group" } });
    } catch (e) {
      if (!gone) send({ type: "failed", error: e as BridgeError });
    }
  }
  function retry(): void {
    send({ type: "reload" });
    void load();
  }
  /** Alle grupper til listen ved siden av (leser samme kort som findGroup; vises først når den er lastet). */
  let all: Grp[] = $state([]);
  async function loadAll(): Promise<void> {
    try {
      const gs = await listGroups(b);
      if (!gone) all = gs;
    } catch { /* listen er et tillegg: feil her skal ikke stoppe gruppevisningen */ }
  }
  // Valg i listen bytter bare rute-id: komponenten lever videre, så last den nye gruppen.
  // svelte-ignore state_referenced_locally
  let shownId = id;
  $effect(() => {
    const n = id;
    untrack(() => { if (n !== shownId) { shownId = n; retry(); } });
  });
  onMount(() => {
    void load();
    void loadAll();
    void loadCloud(b).then((c) => { if (!gone) cloud = c; });
  });
  onDestroy(() => { gone = true; });

  async function makeBest(asset: number): Promise<void> {
    if (view.phase !== "ready") return;
    send({ type: "start" });
    try {
      await b.setKeeper(id, asset);
      const g = await findGroup(b, id);
      if (g) send({ type: "keeperSet", group: g });
      else send({ type: "actionFailed", error: { code: "unknown_card", detail: "group" } });
    } catch (e) {
      send({ type: "actionFailed", error: e as BridgeError });
    }
  }
  async function keepPrimary(): Promise<void> {
    if (view.phase !== "ready" || movable.length === 0 || cloud) return;
    send({ type: "start" });
    try {
      send({ type: "committed", batch: await b.decide(movable.map((c) => c.id), "trash"), moved: movable.length });
    } catch (e) {
      cloud = cloudFromError(e as BridgeError) ?? cloud;
      send({ type: "actionFailed", error: e as BridgeError });
    }
  }
  async function keepAll(): Promise<void> {
    if (view.phase !== "ready" || !view.group) return;
    send({ type: "start" });
    try {
      await b.decide(view.group.cards.map((c) => c.id), "keep");
      navigate({ name: "review", kind: null });
    } catch (e) {
      send({ type: "actionFailed", error: e as BridgeError });
    }
  }
  async function undo(): Promise<void> {
    try { await b.undo(); } catch { /* ingenting å angre */ }
    retry();
  }
  function keydown(e: KeyboardEvent): void {
    if (!view.group || e.ctrlKey || e.altKey || e.metaKey || (e.target as HTMLElement).closest("input, textarea, select")) return;
    const asset = keeperForKey(view.group, e.key);
    if (asset !== null) { e.preventDefault(); void makeBest(asset); }
  }
</script>

<section class="group">
  <PageHeader title={$tr("group.title")} back={{ href: formatHash({ name: "review", kind: null }), label: $tr("group.back") }} />
  {#if cloud}<CloudWarning provider={cloud} />{/if}

  {#if view.phase === "loading"}
    <p role="status">{$tr("common.loading")}</p>
  {:else if view.phase === "error" && !view.group}
    <Banner tone="danger" role="alert">
      <p>{$tr("group.error.body")}</p>
      {#snippet actions()}<Button onclick={retry}>{$tr("common.retry")}</Button>{/snippet}
    </Banner>
  {:else if view.phase === "committed"}
    <Banner tone="ok" role="status">
      <p>{$tr("group.committed", { count: view.moved })}</p>
      {#snippet actions()}<Button onclick={undo}>{$tr("common.undo")}</Button>{/snippet}
    </Banner>
  {:else if view.group}
    {#if view.error}<Banner tone="danger" role="alert"><p>{$tr(cloudFromError(view.error) ? "cloud.error.blocked" : "group.error.body")}</p></Banner>{/if}

    <div class="cq"><div class="layout">
      {#if all.length > 1}
        <ul class="rail" aria-label={$tr("review.title")}>
          {#each all as g (g.id)}
            <li>
              <a class="gl" class:cur={g.id === id} href={formatHash({ name: "group", id: g.id })} aria-current={g.id === id ? "true" : undefined}>
                <span class="mini" aria-hidden="true"></span>
                <span class="txt">
                  <span class="cnt">{$tr("review.group.row", { count: g.cards.length + (g.keeper !== null ? 1 : 0) })}</span>
                  <span class="muted sz">{fmt.bytes(g.bytes)}</span>
                </span>
              </a>
            </li>
          {/each}
        </ul>
      {/if}

      <div class="main">
        <p class="muted lead">{$tr("group.exact.note")}<br />{$tr("group.keys.hint")}</p>

        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <ul class="items" aria-label={$tr("group.title")} onkeydown={keydown}>
          {#each items as it (it.asset)}
            <li class="item" class:best={it.best}>
              <div class="media">
                <div class="img"><Thumb id={it.asset} fit="contain" alt={$tr("group.item.name", { n: it.n, total: items.length })} /><span class="num" aria-hidden="true">{it.n}</span></div>
                {#if it.best}
                  <span class="flag"><Badge label={$tr("group.best")} /></span>
                  <span class="tick" aria-hidden="true"><Icon name="check" /></span>
                {/if}
              </div>
              <p class="name">{$tr("group.item.name", { n: it.n, total: items.length })}</p>
              {#if it.protected}<div class="meta"><Badge tone="warn" label={$tr("group.protected")} /></div>{/if}
              <dl>
                {#if it.bytes !== null}<dt>{$tr("group.info.size")}</dt><dd>{fmt.bytes(it.bytes)}</dd>{/if}
              </dl>
              {#if !it.best}
                <Button size="sm" disabled={busy} ariaKeyshortcuts={it.n <= 9 ? String(it.n) : undefined} onclick={() => makeBest(it.asset)}>{$tr("group.make.best")}</Button>
              {/if}
            </li>
          {/each}
        </ul>
      </div>
    </div></div>

    <BottomBar ariaLabel={$tr("group.title")} announce="" label={$tr("group.keep.primary", { count: movable.length })}
      disabled={busy || (!cloud && movable.length === 0)} blocked={!!cloud} describedby="cloud-why" onclick={keepPrimary}>
      {#snippet secondary()}
        <Button disabled={busy} onclick={keepAll}>{$tr("group.keep.all")}</Button>
      {/snippet}
    </BottomBar>
  {/if}
</section>

<style>
  .group { max-inline-size: var(--w-wide); }
  /* Egen beholder (ikke .group): container-type gir inneslutning som ville gjort den til forelder for den faste bunnlinjen. */
  .cq { container-type: inline-size; }
  .muted { color: var(--c-text-muted); }
  .lead { margin: 0 0 var(--space-4); font-size: var(--fs-sm); max-inline-size: 62ch; }
  :global(.group .banner p) { margin: 0; }
  ul { list-style: none; margin: 0; padding: 0; }
  .layout { display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--space-4); align-items: start; }
  .main { min-inline-size: 0; }

  .rail { display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .gl { display: flex; align-items: center; gap: var(--space-3); min-block-size: var(--target-min); padding: var(--space-2) var(--space-3); color: var(--c-text); text-decoration: none; border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-md); background: var(--c-surface); }
  .gl:hover { background: var(--c-surface-hover); }
  .gl.cur { background: var(--c-nav-active-bg); border: var(--border-width) solid var(--c-border-strong); border-inline-start: 5px solid var(--c-text); font-weight: var(--fw-semibold); }
  .mini { flex: none; inline-size: 2.5rem; block-size: 2rem; border-radius: var(--radius-sm); background: repeating-linear-gradient(45deg, var(--c-surface-sunken), var(--c-surface-sunken) 6px, var(--c-border) 6px, var(--c-border) 7px); }
  .txt { display: flex; flex-direction: column; min-inline-size: 0; }
  .sz { font-size: var(--fs-sm); font-variant-numeric: var(--num); font-weight: var(--fw-regular); }

  .items { display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--space-4); margin: 0; }
  .item { display: flex; flex-direction: column; gap: var(--space-2); min-inline-size: 0; padding: var(--space-3); border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-lg); background: var(--c-surface); box-shadow: var(--shadow-card); }
  .item.best { border: 3px solid var(--c-ok); padding: calc(var(--space-3) - 2px); background: var(--c-ok-bg); }
  .media { position: relative; }
  .img { position: relative; overflow: hidden; aspect-ratio: 4 / 3; border-radius: var(--radius-md); background: repeating-linear-gradient(45deg, var(--c-surface-sunken), var(--c-surface-sunken) 8px, var(--c-border) 8px, var(--c-border) 9px); }
  .num { position: absolute; inset-block-start: var(--space-2); inset-inline-start: var(--space-2); min-inline-size: 1.75rem; padding: 0 var(--space-2); text-align: center; background: var(--c-surface); color: var(--c-text); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-pill); font-weight: var(--fw-semibold); font-variant-numeric: var(--num); }
  .flag { position: absolute; inset-block-end: var(--space-2); inset-inline-start: var(--space-2); max-inline-size: calc(100% - var(--space-4)); }
  .tick { position: absolute; inset-block-start: var(--space-2); inset-inline-end: var(--space-2); display: grid; place-items: center; inline-size: 1.75rem; block-size: 1.75rem; border-radius: var(--radius-pill); background: var(--c-ok); color: var(--c-surface); }
  .tick :global(svg) { inline-size: 1.125rem; block-size: 1.125rem; }
  .name { margin: 0; font-weight: var(--fw-semibold); }
  .meta { display: flex; flex-wrap: wrap; gap: var(--space-1); }
  dl { display: flex; flex-wrap: wrap; gap: var(--space-1) var(--space-3); margin: 0; padding-block-start: var(--space-2); border-block-start: var(--border-width) solid var(--c-border-subtle); font-size: var(--fs-sm); }
  dl:empty { display: none; }
  dt { color: var(--c-text-muted); }
  dd { margin: 0; font-weight: var(--fw-semibold); font-variant-numeric: var(--num); }
  .item :global(.btn) { margin-block-start: auto; }

  /* Containerspørringer (rem følger rot-skriftstørrelsen, media-spørringer gjør ikke det): holder ved 200 % og stor skrift. */
  @container (min-width: 20rem) { .items { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  @container (min-width: 40rem) { .items { grid-template-columns: repeat(auto-fit, minmax(min(100%, var(--tile-min)), 1fr)); } }
  @container (min-width: 52rem) {
    .layout { grid-template-columns: minmax(12rem, 16rem) minmax(0, 1fr); gap: var(--space-5); }
    .rail { flex-direction: column; flex-wrap: nowrap; position: sticky; inset-block-start: var(--space-4); }
    .gl { white-space: normal; }
  }
  @media (forced-colors: active) {
    .item.best { border-color: Highlight; }
    .tick { background: Highlight; color: HighlightText; }
    .gl.cur { border-inline-start-color: Highlight; }
  }
</style>
