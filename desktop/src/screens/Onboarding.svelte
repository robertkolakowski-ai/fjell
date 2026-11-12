<!-- Onboarding (UX-2, PLAN-UX-I18N 3.1): velkommen → mapper → rask skann → resultat → seier. Ingen konto, ingen systemtillatelse.
     Aksentfarge bare på én primærhandling per trinn. All tekst via $tr. Tilstand i onboardingState.ts. -->
<script lang="ts">
  import { onDestroy, tick } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { useBridge } from "../context";
  import { Button, LiveRegion } from "../lib";
  import { fmt, tr } from "../lib/i18n";
  import { navigate } from "../router";
  import FolderDialog from "./FolderDialog.svelte";
  import { addFolder } from "./homeFlow";
  import { quickScan, trashFound } from "./onboardingFlow";
  import { markOnboarded } from "./onboardingGate";
  import { initialOnb, reduceOnb, type OnbEvent, type OnbState } from "./onboardingState";

  interface Props {
    bridge?: CoreBridge;
  }
  let { bridge }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);

  let view: OnbState = $state(initialOnb);
  let days = $state(30);
  let root: HTMLElement | undefined = $state();
  let gone = false;
  onDestroy(() => (gone = true));

  function send(e: OnbEvent): void {
    view = reduceOnb(view, e);
  }
  // Fokus til trinnets overskrift ved trinnbytte (skjermleser annonserer nytt trinn).
  let lastKind = "";
  $effect(() => {
    const k = view.kind;
    if (k === lastKind) return;
    const first = lastKind === "";
    lastKind = k;
    if (!first) void tick().then(() => root?.querySelector<HTMLElement>("h1")?.focus());
  });

  let library: string | null = $state(null);
  let folderOpen = $state(false);
  $effect(() => {
    if (view.kind === "chooseFolders") void b.getLibrary().then((l) => { if (!gone) library = l; }, () => undefined);
  });
  async function chooseFolder(path: string): Promise<boolean> {
    const r = await addFolder(b, path, false);
    if (!r.ok) return false;
    library = r.path;
    folderOpen = false;
    return true;
  }

  async function start(): Promise<void> {
    // Uten valgt mappe finnes ingenting å skanne: be om en først.
    let lib: string | null = null;
    try { lib = await b.getLibrary(); } catch { /* feil kommer fra skannet under */ }
    if (lib === null && library === null) { folderOpen = true; return; }
    void markOnboarded(b);
    send({ type: "startScan" });
    const r = await quickScan(b, {
      onFound: (found) => send({ type: "progress", found }),
      onSlow: () => send({ type: "slow" }),
      cancelled: () => gone,
    });
    if (gone) return;
    if (r.kind === "denied") return send({ type: "scanDenied" });
    if (r.kind === "failed") return send({ type: "scanFailed" });
    try { days = (await b.getState()).trashDays; } catch { /* standardverdi gjelder */ }
    send({ type: "exactReady", ...r.found });
  }

  async function trash(): Promise<void> {
    if (view.kind !== "result") return;
    const ids = view.ids;
    try {
      await trashFound(b, ids);
      send({ type: "trashed" });
    } catch {
      send({ type: "trashFailed" });
    }
  }
  function requestTrash(): void {
    send({ type: "requestTrash" });
    // Under 50 bilder og 1 GB: ingen dialog (PLAN 3.1), flytt straks.
    if (view.kind === "result" && view.busy) void trash();
  }
  function confirmTrash(): void {
    send({ type: "confirmTrash" });
    if (view.kind === "result" && view.busy) void trash();
  }
  async function undo(): Promise<void> {
    send({ type: "undo" });
    try {
      await b.undo();
      send({ type: "undone" });
    } catch {
      send({ type: "undoFailed" });
    }
  }
  const goReview = () => navigate({ name: "review", kind: "exact_dup" });
  const goHome = () => navigate({ name: "home" });
</script>

<section bind:this={root} class="onb">
  {#if view.kind === "welcome"}
    <h1 tabindex="-1">{$tr("onb.welcome.title")}</h1>
    <p>{$tr("onb.welcome.body")}</p>
    <div class="row"><Button variant="primary" onclick={() => send({ type: "begin" })}>{$tr("onb.start.button")}</Button></div>
  {:else if view.kind === "chooseFolders"}
    <h1 tabindex="-1">{$tr("onb.folders.title")}</h1>
    <p>{$tr("onb.folders.privacy")}</p>
    {#if view.failed}<p role="alert">{$tr("err.generic")}</p>{/if}
    <ul class="folders"><li>{library ?? $tr("home.folder.none")}</li></ul>
    <div class="row">
      <Button onclick={() => (folderOpen = true)}>{$tr("onb.folders.add")}</Button>
      <Button variant="primary" onclick={start}>{$tr("onb.folders.start")}</Button>
    </div>
    <FolderDialog open={folderOpen} onSubmit={chooseFolder} onClose={() => (folderOpen = false)} />
  {:else if view.kind === "noAccess"}
    <h1 tabindex="-1">{$tr("onb.noaccess.title")}</h1>
    <p>{$tr("onb.noaccess.body")}</p>
    <div class="row"><Button variant="primary" onclick={() => send({ type: "chooseAgain" })}>{$tr("onb.noaccess.choose")}</Button></div>
  {:else if view.kind === "scanning" || view.kind === "partial"}
    <h1 tabindex="-1">{view.kind === "partial" ? $tr("onb.partial.title") : $tr("onb.scan.title")}</h1>
    <p class="live-count" aria-hidden="true">{$tr("onb.scan.found", { count: view.found })}</p>
    <LiveRegion message={$tr("onb.scan.found", { count: view.found })} delay={1000} />
    <p class="muted">{view.kind === "partial" ? $tr("onb.partial.body") : $tr("onb.scan.metadata")}</p>
    {#if view.kind === "partial"}
      <div class="row">
        <Button onclick={goReview}>{$tr("onb.result.review")}</Button>
        <Button onclick={goHome}>{$tr("onb.done.home")}</Button>
      </div>
    {/if}
  {:else if view.kind === "nothingFound"}
    <h1 tabindex="-1">{$tr("onb.scan.title")}</h1>
    <p>{$tr("onb.nothing.body")}</p>
    <div class="row"><Button variant="primary" onclick={goHome}>{$tr("onb.done.home")}</Button></div>
  {:else if view.kind === "result"}
    <h1 tabindex="-1">{$tr("onb.result.summary", { count: view.count, size: fmt.bytes(view.bytes) })}</h1>
    {#if view.failed}<p role="alert">{$tr("review.error.body")}</p>{/if}
    {#if view.confirming}
      <!-- MassAck: avkrysning kreves, aldri forhåndskrysset. Esc/Avbryt lukker uten endring. -->
      <div class="ack" role="group" aria-labelledby="ack-title">
        <h2 id="ack-title">{$tr("review.confirm.title")}</h2>
        <p>{$tr("review.confirm.body", { days })}</p>
        <label class="check">
          <input type="checkbox" checked={view.acked} onchange={(e) => send({ type: "setAck", acked: e.currentTarget.checked })} />
          <span>{$tr("review.confirm.ack")}</span>
        </label>
        <div class="row">
          <Button onclick={() => send({ type: "cancelConfirm" })}>{$tr("common.cancel")}</Button>
          <Button variant="primary" disabled={!view.acked} onclick={confirmTrash}>{$tr("review.confirm.go")}</Button>
        </div>
      </div>
    {:else}
      <div class="row">
        <Button variant="primary" disabled={view.busy} onclick={requestTrash}>{$tr("onb.result.primary", { count: view.count })}</Button>
        <Button disabled={view.busy} onclick={goReview}>{$tr("onb.result.review")}</Button>
      </div>
    {/if}
  {:else if view.kind === "done"}
    <h1 tabindex="-1">{$tr("onb.done.title", { count: view.count })}</h1>
    <p>{$tr("onb.done.body", { size: fmt.bytes(view.bytes) })}</p>
    <LiveRegion message={$tr("onb.done.title", { count: view.count })} />
    {#if view.failed}<p role="alert">{$tr("err.generic")}</p>{/if}
    <div class="row">
      <Button disabled={view.busy} onclick={undo}>{$tr("common.undo")}</Button>
      <Button variant="primary" disabled={view.busy} onclick={goHome}>{$tr("onb.done.home")}</Button>
    </div>
  {/if}
</section>

<style>
  .onb { max-inline-size: 40rem; }
  h1 { margin-block: 0 var(--space-3); font-size: var(--fs-xl); line-height: var(--lh-tight); }
  h1:focus { outline: none; }
  h2 { margin: 0 0 var(--space-2); font-size: var(--fs-lg); }
  p { margin-block: 0 var(--space-4); }
  .muted { color: var(--c-text-muted); }
  .live-count { font-size: var(--fs-lg); font-weight: var(--fw-semibold); }
  .row { display: flex; flex-wrap: wrap; gap: var(--space-3); margin-block-start: var(--space-4); }
  .folders { list-style: none; margin: 0 0 var(--space-4); padding: var(--space-3) var(--space-4); border: var(--border-width) solid var(--c-border); border-radius: var(--radius-md); background: var(--c-surface); }
  .ack { padding: var(--space-4) var(--space-5); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-lg); background: var(--c-surface); }
  .check { display: flex; gap: var(--space-3); align-items: flex-start; min-block-size: var(--target-min); }
  .check input { inline-size: 1.25rem; block-size: 1.25rem; margin-block-start: 0.15rem; flex: none; }
</style>
