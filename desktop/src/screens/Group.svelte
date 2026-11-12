<!-- Gruppevisning (UX-3, PLAN-UX-I18N 3.4): like bilder side om side, «Beste» forhåndsmerket (ikon + tekst + ramme),
     «Behold dette, flytt N» (primær), «Behold alle», «Gjør dette til beste» (ett trykk eller tast 1–9 når gruppeområdet har fokus).
     Beskyttede bilder flyttes aldri. Tilstand i groupState.ts. -->
<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import type { BridgeError } from "../bridge/types";
  import { useBridge } from "../context";
  import { Badge, Button, CloudWarning } from "../lib";
  import { fmt, tr } from "../lib/i18n";
  import { formatHash, navigate } from "../router";
  import { cloudFromError, loadCloud } from "./cloud";
  import {
    findGroup, groupItems, initialGroup, keeperForKey, movableCards, reduceGroup, type GroupEvent, type GroupState,
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
  onMount(() => {
    void load();
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
  <h1 tabindex="-1">{$tr("group.title")}</h1>
  <p><a href={formatHash({ name: "review", kind: null })}>{$tr("group.back")}</a></p>
  {#if cloud}<CloudWarning provider={cloud} />{/if}

  {#if view.phase === "loading"}
    <p role="status">{$tr("common.loading")}</p>
  {:else if view.phase === "error" && !view.group}
    <p role="alert">{$tr("group.error.body")}</p>
    <Button onclick={retry}>{$tr("common.retry")}</Button>
  {:else if view.phase === "committed"}
    <p role="status">{$tr("group.committed", { count: view.moved })}</p>
    <Button onclick={undo}>{$tr("common.undo")}</Button>
  {:else if view.group}
    {#if view.error}<p role="alert">{$tr(cloudFromError(view.error) ? "cloud.error.blocked" : "group.error.body")}</p>{/if}
    <p class="muted">{$tr("group.exact.note")}</p>
    <p class="muted">{$tr("group.keys.hint")}</p>

    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <ul class="items" aria-label={$tr("group.title")} onkeydown={keydown}>
      {#each items as it (it.asset)}
        <li class="item" class:best={it.best}>
          <div class="img" aria-hidden="true"><span class="num">{it.n}</span></div>
          <p class="name">{$tr("group.item.name", { n: it.n, total: items.length })}</p>
          <div class="meta">
            {#if it.best}<Badge label={$tr("group.best")} />{/if}
            {#if it.protected}<Badge tone="warn" label={$tr("group.protected")} />{/if}
          </div>
          <dl>
            {#if it.bytes !== null}<dt>{$tr("group.info.size")}</dt><dd>{fmt.bytes(it.bytes)}</dd>{/if}
          </dl>
          {#if !it.best}
            <Button disabled={busy} ariaKeyshortcuts={it.n <= 9 ? String(it.n) : undefined} onclick={() => makeBest(it.asset)}>{$tr("group.make.best")}</Button>
          {/if}
        </li>
      {/each}
    </ul>

    <div class="actions">
      <Button variant="primary" disabled={busy || (!cloud && movable.length === 0)} ariaDisabled={!!cloud} ariaDescribedby="cloud-why" onclick={keepPrimary}>{$tr("group.keep.primary", { count: movable.length })}</Button>
      <Button disabled={busy} onclick={keepAll}>{$tr("group.keep.all")}</Button>
    </div>
  {/if}
</section>

<style>
  .group { max-inline-size: 60rem; }
  h1 { margin-block: 0 var(--space-3); font-size: var(--fs-xl); line-height: var(--lh-tight); }
  h1:focus { outline: none; }
  .muted { color: var(--c-text-muted); margin-block: 0 var(--space-2); }
  .items { list-style: none; margin: var(--space-3) 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fit, minmax(12rem, 1fr)); gap: var(--space-3); }
  .item { display: flex; flex-direction: column; gap: var(--space-2); padding: var(--space-3); border: var(--border-width) solid var(--c-border); border-radius: var(--radius-md); background: var(--c-surface); }
  .item.best { border: 3px solid var(--c-selected); padding: calc(var(--space-3) - 2px); }
  .img { position: relative; aspect-ratio: 4 / 3; border-radius: var(--radius-sm); background: repeating-linear-gradient(45deg, var(--c-surface-sunken), var(--c-surface-sunken) 8px, var(--c-border) 8px, var(--c-border) 9px); }
  .num { position: absolute; inset-block-start: var(--space-2); inset-inline-start: var(--space-2); min-inline-size: 1.75rem; padding: 0 var(--space-2); text-align: center; background: var(--c-surface); color: var(--c-text); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-pill); font-weight: var(--fw-semibold); }
  .name { margin: 0; font-weight: var(--fw-semibold); }
  .meta { display: flex; flex-wrap: wrap; gap: var(--space-1); }
  dl { display: grid; grid-template-columns: auto 1fr; gap: var(--space-1) var(--space-3); margin: 0; }
  dt { color: var(--c-text-muted); }
  dd { margin: 0; }
  .actions { display: flex; flex-wrap: wrap; gap: var(--space-3); }
</style>
