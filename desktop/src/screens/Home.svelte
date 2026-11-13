<!-- Hjem (UX-2, PLAN-UX-I18N 3.2; M10 Loft): kompakt ring + nøkkeltall i ett kort, kategorier som StatTile-rutenett, mappe og skann som statuslinjer, bannere. Ring («X kan ryddes»), kategorikort, Rask opprydding (eneste aksentknapp), skannstatus
     med Pause/Fortsett/Avbryt, «Legg til mappe», papirkurvlenke, utløpt-banner. Tilstand i homeState.ts, effekter i homeFlow.ts.
     Ringen har alltid tekstekvivalent. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { KIND_CODES, type BridgeError, type KindCode } from "../bridge/types";
  import { useBridge } from "../context";
  import { Banner, Button, Card, CloudWarning, LiveRegion, PageHeader, Ring, StatTile } from "../lib";
  import { fmt, tr } from "../lib/i18n";
  import { formatHash, navigate } from "../router";
  import { pendingUpdate } from "./updateFlow";
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
  <PageHeader title={$tr("home.title")} />

  {#if view.kind === "loading"}
    <p role="status">{$tr("common.loading")}</p>
  {:else if view.kind === "error"}
    <p role="alert">{$tr("home.error.body")}</p>
    <Button onclick={retry}>{$tr("common.retry")}</Button>
  {:else if summary}
    {#if summary.cloudSync}<CloudWarning provider={summary.cloudSync} />{/if}

    {#if $pendingUpdate}
      <Banner tone="info" role="status">
        <p>{$tr("home.update.banner", { version: $pendingUpdate.version })}</p>
        {#snippet actions()}<a href={formatHash({ name: "settings" })}>{$tr("nav.settings")}</a>{/snippet}
      </Banner>
    {/if}
    {#if summary.trashExpired > 0}
      <Banner tone="warn" role="status">
        <p>{$tr("home.expired.banner", { count: summary.trashExpired })}</p>
        {#snippet actions()}<a href={formatHash({ name: "trash" })}>{$tr("nav.trash")}</a>{/snippet}
      </Banner>
    {/if}

    <div class="strip folder">
      <p class="path">{library ? $tr("home.folder.current", { path: library }) : $tr("home.folder.none")}</p>
      <Button size="sm" onclick={() => (folderOpen = true)}>{$tr("home.folder.add")}</Button>
    </div>

    {#if view.kind === "scanning" || view.kind === "paused"}
      <div class="strip scan">
        {#if view.kind === "scanning"}
          <p class="path">{$tr("home.scan.progress", { done: fmt.number(view.done), total: fmt.number(view.total) })}</p>
          <progress aria-hidden="true" max={Math.max(1, view.total)} value={view.done}></progress>
          <LiveRegion message={$tr("a11y.live.scan", { done: fmt.number(view.done), total: fmt.number(view.total) })} delay={2000} />
        {:else}
          <p class="path" role="status">{$tr("home.scan.paused")}</p>
        {/if}
        <div class="ctl" role="group" aria-label={$tr("home.scan.controls")}>
          {#if view.kind === "scanning"}
            <Button size="sm" onclick={() => void control("pause")}>{$tr("home.scan.pause")}</Button>
          {:else}
            <Button size="sm" onclick={() => void control("resume")}>{$tr("home.scan.resume")}</Button>
          {/if}
          <Button size="sm" onclick={() => void control("cancel")}>{$tr("home.scan.cancel")}</Button>
        </div>
      </div>
    {/if}
    {#if note === "cancelled"}<p class="muted" role="status">{$tr("home.scan.cancelled")}</p>{/if}
    {#if note === "error"}<p role="alert">{$tr("home.scan.error")}</p>{/if}

    {#if view.kind === "allClean"}
      <Card>
        <p class="clean">{$tr("home.clean.body")}</p>
        <a href={formatHash({ name: "trash" })}>{$tr("home.trash.link", { count: summary.trashCount })}</a>
      </Card>
    {:else if neverScanned(summary) && view.kind === "ready"}
      <Card>
        <p class="idle">{$tr("home.idle.body")}</p>
        {#if library}
          <Button variant="primary" onclick={() => void control("start")}>{$tr("home.scan.start")}</Button>
        {/if}
        <p class="after"><a href={formatHash({ name: "trash" })}>{$tr("home.trash.link", { count: summary.trashCount })}</a></p>
      </Card>
    {:else}
      <div class="main">
        <Card>
          <div class="hero">
            <Ring
              fraction={ringFraction(summary)}
              value={fmt.bytes(summary.reclaimableBytes)}
              label={$tr("home.ring.label", { size: fmt.bytes(summary.reclaimableBytes), trash: fmt.bytes(summary.trashBytes) })}
            />
            <div class="facts">
              <p class="caption">{$tr("home.ring.caption", { size: fmt.bytes(summary.reclaimableBytes) })}</p>
              <p class="muted">{$tr("home.ring.trash", { size: fmt.bytes(summary.trashBytes) })}</p>
              <div class="acts">
                <Button variant="primary" disabled={pendingCount(summary) === 0} onclick={() => navigate({ name: "cards", kind: null })}>{$tr("home.quick.button")}</Button>
                <a href={formatHash({ name: "trash" })}>{$tr("home.trash.link", { count: summary.trashCount })}</a>
              </div>
            </div>
          </div>
        </Card>
        <ul class="cats">
          {#each KIND_CODES as kind (kind)}
            {@const k = kindOf(summary, kind)}
            <li>
              <StatTile label={$tr(catKey[kind])} value={fmt.bytes(k.bytes)} hint={$tr("common.count.photos", { count: k.count })} href={formatHash({ name: "review", kind })} />
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  {/if}
  <FolderDialog bridge={b} open={folderOpen} onSubmit={chooseFolder} onClose={() => (folderOpen = false)} />
</section>

<style>
  .home { container-type: inline-size; max-inline-size: var(--w-wide); }
  p { margin-block: 0 var(--space-3); }
  .muted { color: var(--c-text-muted); }
  .clean, .idle { font-size: var(--fs-lg); }
  .after { margin-block: var(--space-4) 0; }
  .strip { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-3); margin-block-end: var(--space-4); padding: var(--space-2) var(--space-3) var(--space-2) var(--space-4); background: var(--c-surface-sunken); border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-md); }
  .strip p { margin: 0; }
  .path { flex: 1 1 14rem; min-inline-size: 0; overflow-wrap: anywhere; }
  .folder .path { color: var(--c-text-muted); }
  progress { flex: 1 1 8rem; max-inline-size: 16rem; block-size: var(--space-2); accent-color: var(--c-accent); }
  .ctl { display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .main { display: grid; gap: var(--space-4); align-items: start; }
  .hero { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-3) var(--space-5); }
  .facts { flex: 1 1 12rem; min-inline-size: 0; }
  .caption { margin-block: 0 var(--space-1); font-size: var(--fs-lg); font-weight: var(--fw-semibold); font-variant-numeric: var(--num); }
  .acts { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-4); margin-block-start: var(--space-4); }
  .cats { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--space-4); grid-template-columns: repeat(auto-fit, minmax(var(--card-min), 1fr)); }
  /* Bredt arbeidsflate: ring-kort til venstre, kategorier 2×2 til høyre. */
  @container (min-width: 52rem) {
    .main { grid-template-columns: minmax(15rem, 18rem) minmax(0, 1fr); }
    .hero { flex-direction: column; text-align: center; }
    .facts { flex: none; inline-size: 100%; }
    .acts { flex-direction: column; }
  }
</style>
