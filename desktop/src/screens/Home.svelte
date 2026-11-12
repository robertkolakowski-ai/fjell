<!-- Hjem (UX-2, PLAN-UX-I18N 3.2): ring («X kan ryddes»), kategorikort, Rask opprydding (eneste aksentknapp), skannstatus
     med Pause/Fortsett/Avbryt, «Legg til mappe», papirkurvlenke, utløpt-banner. Tilstand i homeState.ts, effekter i homeFlow.ts.
     Ringen har alltid tekstekvivalent. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { KIND_CODES, type BridgeError, type KindCode } from "../bridge/types";
  import { useBridge } from "../context";
  import { Button, CloudWarning, LiveRegion, Ring } from "../lib";
  import { fmt, tr } from "../lib/i18n";
  import { formatHash, navigate } from "../router";
  import FolderDialog from "./FolderDialog.svelte";
  import { addFolder, controlScan, type ScanNote } from "./homeFlow";
  import { initialHome, kindOf, neverScanned, pendingCount, reduceHome, ringFraction, type HomeEvent, type HomeView } from "./homeState";

  interface Props {
    bridge?: CoreBridge;
    /** Pollingintervall under skann (ms). */
    pollMs?: number;
  }
  let { bridge, pollMs = 1000 }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);

  let view: HomeView = $state(initialHome);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let gone = false;

  const catKey = {
    exact_dup: "home.cat.exact_dup", visual_dup: "home.cat.visual_dup",
    blurry: "home.cat.blurry", screenshot: "home.cat.screenshot",
  } as const satisfies Record<KindCode, string>;

  function send(e: HomeEvent): void {
    view = reduceHome(view, e);
  }
  async function load(): Promise<void> {
    clearTimeout(timer);
    try {
      send({ type: "loaded", summary: await b.getState() });
    } catch (e) {
      send({ type: "failed", error: e as BridgeError });
    }
    try { library = await b.getLibrary(); } catch { /* mappe vises bare når den er kjent */ }
    if (!gone && view.kind === "scanning") timer = setTimeout(load, pollMs);
  }
  let library: string | null = $state(null);
  let note: ScanNote = $state(null);
  let folderOpen = $state(false);

  async function control(action: "pause" | "resume" | "cancel" | "start"): Promise<void> {
    note = await controlScan(b, action);
    await load();
  }
  async function chooseFolder(path: string): Promise<boolean> {
    const r = await addFolder(b, path);
    if (!r.ok) return false;
    folderOpen = false;
    note = null;
    await load();
    return true;
  }
  function retry(): void {
    send({ type: "retry" });
    void load();
  }
  onMount(() => void load());
  onDestroy(() => { gone = true; clearTimeout(timer); });

  const summary = $derived(view.kind === "scanning" || view.kind === "ready" || view.kind === "allClean" ? view.summary : null);
</script>

<section class="home">
  <h1 tabindex="-1">{$tr("home.title")}</h1>

  {#if view.kind === "loading"}
    <p role="status">{$tr("common.loading")}</p>
  {:else if view.kind === "error"}
    <p role="alert">{$tr("home.error.body")}</p>
    <Button onclick={retry}>{$tr("common.retry")}</Button>
  {:else if summary}
    {#if summary.cloudSync}<CloudWarning provider={summary.cloudSync} />{/if}

    {#if summary.trashExpired > 0}
      <div class="banner" role="status">
        <p>{$tr("home.expired.banner", { count: summary.trashExpired })}</p>
        <a href={formatHash({ name: "trash" })}>{$tr("nav.trash")}</a>
      </div>
    {/if}

    {#if view.kind === "scanning" || view.kind === "paused"}
      {#if view.kind === "scanning"}
        <p class="muted">{$tr("home.scan.progress", { done: fmt.number(view.done), total: fmt.number(view.total) })}</p>
        <LiveRegion message={$tr("a11y.live.scan", { done: fmt.number(view.done), total: fmt.number(view.total) })} delay={2000} />
      {:else}
        <p class="muted" role="status">{$tr("home.scan.paused")}</p>
      {/if}
      <div class="row ctl" role="group" aria-label={$tr("home.scan.controls")}>
        {#if view.kind === "scanning"}
          <Button onclick={() => void control("pause")}>{$tr("home.scan.pause")}</Button>
        {:else}
          <Button onclick={() => void control("resume")}>{$tr("home.scan.resume")}</Button>
        {/if}
        <Button onclick={() => void control("cancel")}>{$tr("home.scan.cancel")}</Button>
      </div>
    {/if}
    {#if note === "cancelled"}<p class="muted" role="status">{$tr("home.scan.cancelled")}</p>{/if}
    {#if note === "error"}<p role="alert">{$tr("home.scan.error")}</p>{/if}

    {#if view.kind === "allClean"}
      <p class="clean">{$tr("home.clean.body")}</p>
    {:else if neverScanned(summary) && view.kind === "ready"}
      <p>{$tr("home.idle.body")}</p>
      {#if library}
        <div class="row"><Button onclick={() => void control("start")}>{$tr("home.scan.start")}</Button></div>
      {/if}
    {:else}
      <div class="ring">
        <Ring
          fraction={ringFraction(summary)}
          value={fmt.bytes(summary.reclaimableBytes)}
          label={$tr("home.ring.label", { size: fmt.bytes(summary.reclaimableBytes), trash: fmt.bytes(summary.trashBytes) })}
        />
        <p class="caption">{$tr("home.ring.caption", { size: fmt.bytes(summary.reclaimableBytes) })}</p>
        <p class="muted">{$tr("home.ring.trash", { size: fmt.bytes(summary.trashBytes) })}</p>
      </div>
      <div class="row">
        <Button variant="primary" disabled={pendingCount(summary) === 0} onclick={() => navigate({ name: "cards", kind: null })}>{$tr("home.quick.button")}</Button>
      </div>
      <ul class="cats">
        {#each KIND_CODES as kind (kind)}
          {@const k = kindOf(summary, kind)}
          <li>
            <a class="cat" href={formatHash({ name: "review", kind })}>
              <span class="name">{$tr(catKey[kind])}</span>
              <span class="sum">{$tr("home.cat.summary", { count: k.count, size: fmt.bytes(k.bytes) })}</span>
              <span class="chev" aria-hidden="true">›</span>
            </a>
          </li>
        {/each}
      </ul>
    {/if}

    <p><a href={formatHash({ name: "trash" })}>{$tr("home.trash.link", { count: summary.trashCount })}</a></p>

    <div class="folder">
      <p class="muted">{library ? $tr("home.folder.current", { path: library }) : $tr("home.folder.none")}</p>
      <Button onclick={() => (folderOpen = true)}>{$tr("home.folder.add")}</Button>
    </div>
  {/if}
  <FolderDialog bridge={b} open={folderOpen} onSubmit={chooseFolder} onClose={() => (folderOpen = false)} />
</section>

<style>
  .home { max-inline-size: 40rem; }
  h1 { margin-block: 0 var(--space-4); font-size: var(--fs-xl); line-height: var(--lh-tight); }
  h1:focus { outline: none; }
  p { margin-block: 0 var(--space-3); }
  .muted { color: var(--c-text-muted); }
  .clean { font-size: var(--fs-lg); }
  .ring { display: flex; flex-direction: column; align-items: center; text-align: center; margin-block-end: var(--space-4); }
  .caption { margin-block: var(--space-2) var(--space-1); font-size: var(--fs-lg); font-weight: var(--fw-semibold); }
  .row { display: flex; justify-content: center; margin-block-end: var(--space-5); }
  .ctl { gap: var(--space-3); flex-wrap: wrap; }
  .folder { margin-block-start: var(--space-5); padding-block-start: var(--space-4); border-block-start: var(--border-width) solid var(--c-border); }
  .folder p { overflow-wrap: anywhere; }
  .banner { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-4); padding: var(--space-3) var(--space-4); margin-block-end: var(--space-4); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-md); background: var(--c-surface); }
  .banner p { margin: 0; }
  .cats { list-style: none; margin: 0 0 var(--space-4); padding: 0; display: grid; gap: var(--space-3); }
  .cat { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-1) var(--space-3); min-block-size: var(--target-min); padding: var(--space-3) var(--space-4); color: var(--c-text); text-decoration: none; background: var(--c-surface); border: var(--border-width) solid var(--c-border); border-radius: var(--radius-lg); box-shadow: var(--shadow-sm); }
  .cat:hover { background: var(--c-surface-sunken); }
  .name { font-weight: var(--fw-semibold); flex: 1 1 10rem; }
  .sum { color: var(--c-text-muted); }
  .chev { margin-inline-start: auto; }
</style>
