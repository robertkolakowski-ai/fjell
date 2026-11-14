<!-- Oppdateringer (M9): versjon, «Se etter oppdatering», status i live region, «Installer og start på nytt» med fremdrift,
     bryter «Sjekk automatisk ved oppstart» (standard av). Sjekk skjer bare på knappetrykk eller når auto er på (App.svelte).
     Aldri automatisk installasjon, og ingen installasjon mens skanning pågår (aria-disabled + forklaring, knappen beholder fokus).
     Reservevei: feiler installasjonen (eller ble en tidligere installasjon aldri fullført), vises adressen til siste utgivelse med
     «Kopier lenke». Appen åpner aldri adressen selv (ingen opener-/shell-plugin).
     Tilstand i updateState.ts, flyt i updateFlow.ts. Notatene fra utgivelsen vises som ren tekst (aldri som HTML). -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { useBridge } from "../context";
  import { Button, Card, LiveRegion } from "../lib";
  import { fmt, tr } from "../lib/i18n";
  import { UpdateSession } from "./updateFlow";
  import { initialUpdate, installBlocked, progressPercent, showManual, type UpdateView } from "./updateState";

  interface Props {
    bridge?: CoreBridge;
    /** Hvor ofte skannstatus leses på nytt mens en ny versjon venter (ms). */
    pollMs?: number;
  }
  let { bridge, pollMs = 1500 }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new UpdateSession(b, (s) => (view = s));
  let view: UpdateView = $state(initialUpdate);
  let timer: ReturnType<typeof setInterval> | undefined;
  let urlEl: HTMLElement | undefined = $state();

  const found = $derived(view.found);
  const blocked = $derived(installBlocked(view));
  const busy = $derived(view.status === "checking" || view.status === "installing" || view.status === "restarting");
  const showInstall = $derived(found !== null && (view.status === "available" || view.status === "installing" || view.status === "restarting" || view.status === "installFailed"));
  const manual = $derived(showManual(view));
  const incomplete = $derived(view.incomplete !== null && view.status !== "installing" && view.status !== "restarting" && view.status !== "installFailed");
  const copyMessage = $derived(view.copied === "ok" ? $tr("update.manual.copied") : view.copied === "failed" ? $tr("update.manual.failed") : "");
  const pct = $derived(progressPercent(view.done, view.total));
  const released = $derived.by(() => {
    const t = found?.date ? Date.parse(found.date) : NaN;
    return Number.isNaN(t) ? null : fmt.date(t);
  });

  /** Melding til live region og synlig status. Ikke fremdriften: den leses via progressbar, ikke hvert steg. */
  const message = $derived.by((): string => {
    switch (view.status) {
      case "checking": return $tr("update.status.checking");
      case "none": return $tr("update.status.none");
      case "available": return $tr("update.status.available", { version: found?.version ?? "" });
      case "error": return view.error?.code === "unsupported" ? $tr("update.error.unsupported") : $tr("update.error.check");
      case "installing": return $tr("update.status.installing", { version: found?.version ?? "" });
      case "restarting": return $tr("update.status.restarting");
      case "installFailed": return view.error?.code === "update_install_stalled" ? $tr("update.error.stalled") : $tr("update.error.install");
      default: return "";
    }
  });
  const progressText = $derived(
    pct === null
      ? $tr("update.progress.unknown", { done: fmt.bytes(view.done) })
      : $tr("update.progress.known", { percent: fmt.percent(pct / 100), done: fmt.bytes(view.done), total: fmt.bytes(view.total ?? 0) }),
  );

  function check(): void { if (!busy) void session.check(); }
  function install(): void { if (!busy && !blocked) void session.install(); }
  /** Kopierer lenken. Går det ikke, merkes lenken så brukeren kan kopiere selv (Ctrl+C). */
  async function copy(): Promise<void> {
    if (!(await session.copyLink()) && urlEl) globalThis.getSelection?.()?.selectAllChildren(urlEl);
  }

  // Mens en ny versjon venter: les skannstatus på nytt, så knappen åpnes når skannet er pauset, avbrutt eller ferdig.
  $effect(() => {
    clearInterval(timer);
    if (view.status === "available" || view.status === "installFailed") timer = setInterval(() => void session.refreshScan(), pollMs);
    return () => clearInterval(timer);
  });
  onMount(() => void session.load());
  onDestroy(() => { clearInterval(timer); session.dispose(); });
</script>

<Card>
<section class="update" aria-labelledby="update-h">
  <header class="hd">
    <h2 id="update-h">{$tr("update.title")}</h2>
    {#if view.version !== null}<p class="ver">{$tr("update.version", { version: view.version })}</p>{/if}
  </header>
  <LiveRegion message={message} />

  <div class="row">
    <Button ariaDisabled={busy} onclick={check}>{$tr("update.check")}</Button>
    {#if showInstall}
      <Button variant="primary" ariaDisabled={busy || blocked} ariaDescribedby={blocked ? "update-blocked" : undefined} onclick={install}>{$tr("update.install")}</Button>
    {/if}
    {#if message !== ""}
      <p class="status" class:err={view.status === "error" || view.status === "installFailed"}>{message}</p>
    {/if}
  </div>

  {#if blocked}
    <p id="update-blocked" class="muted">{$tr("update.blocked")}</p>
  {/if}

  {#if view.status === "available" && !blocked}
    <p class="muted">{$tr("update.install.note")}</p>
  {/if}

  {#if incomplete}
    <p class="status err">{$tr("update.error.incomplete", { version: view.incomplete ?? "", current: view.version ?? "" })}</p>
  {/if}

  {#if manual}
    <div class="manual">
      <h3>{$tr("update.manual.title")}</h3>
      <p>{$tr("update.manual.text")}</p>
      <div class="link">
        <p bind:this={urlEl} class="url">{view.downloadUrl ?? ""}</p>
        <Button onclick={() => void copy()}>{$tr("update.manual.copy")}</Button>
      </div>
      <LiveRegion message={copyMessage} />
      {#if copyMessage !== ""}<p class="muted">{copyMessage}</p>{/if}
    </div>
  {/if}

  {#if view.status === "installing" || view.status === "restarting"}
    <div class="progress">
      <div
        class="track" role="progressbar" aria-label={$tr("update.progress.label")}
        aria-valuemin="0" aria-valuemax="100" aria-valuenow={view.status === "restarting" ? 100 : (pct ?? undefined)}
        aria-valuetext={progressText}
      >
        <div class="bar" class:indet={pct === null && view.status === "installing"} style:inline-size={view.status === "restarting" ? "100%" : pct === null ? undefined : `${pct}%`}></div>
      </div>
      <p class="muted">{progressText}</p>
    </div>
  {/if}

  {#if found && (view.status === "available" || view.status === "installFailed") && (found.notes || released)}
    <div class="notes">
      <h3>{$tr("update.notes.title")}</h3>
      {#if released}<p class="muted">{$tr("update.released", { date: released })}</p>{/if}
      {#if found.notes}<p class="text">{found.notes}</p>{/if}
    </div>
  {/if}

  <div class="auto">
    <label class="switch">
      <input
        type="checkbox" role="switch" checked={view.auto} disabled={view.autoSaving} aria-describedby="update-auto-note"
        onchange={(e) => void session.setAuto(e.currentTarget.checked)}
      />
      <span>{$tr("update.auto.label")}</span>
    </label>
    <p id="update-auto-note" class="muted">{$tr("update.auto.note")}</p>
    {#if view.autoFailed}<p role="alert">{$tr("update.auto.error")}</p>{/if}
  </div>
</section>
</Card>

<style>
  .update { display: grid; gap: var(--space-3); }
  .hd { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: var(--space-1) var(--space-4); }
  h2 { margin: 0; font-size: var(--fs-lg); line-height: var(--lh-tight); }
  h3 { margin: 0; font-size: var(--fs-md); }
  p { margin: 0; }
  .ver { color: var(--c-text-muted); font-size: var(--fs-sm); font-variant-numeric: var(--num); }
  .muted { color: var(--c-text-muted); font-size: var(--fs-sm); }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-3); }
  .status { flex: 1 1 14rem; min-inline-size: 0; }
  .status.err { font-weight: var(--fw-semibold); }
  .text { white-space: pre-line; overflow-wrap: anywhere; }
  .manual { display: grid; gap: var(--space-2); padding: var(--space-3) var(--space-4); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-md); }
  .link { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-3); }
  /* Hele lenken er alltid synlig (brytes ved behov) og merkes med ett klikk. */
  .url {
    flex: 1 1 16rem; min-inline-size: 0; padding: var(--space-2) var(--space-3); font-size: var(--fs-sm); overflow-wrap: anywhere; user-select: all;
    background: var(--c-surface-sunken); border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-sm);
  }
  .notes { display: grid; gap: var(--space-1); padding: var(--space-3) var(--space-4); border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-md); background: var(--c-surface-sunken); }
  .progress { display: grid; gap: var(--space-1); }
  .track { block-size: 0.75rem; overflow: hidden; border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-pill); background: var(--c-surface-sunken); }
  .bar { block-size: 100%; background: var(--c-accent); }
  .bar.indet { inline-size: 40%; animation: slide 1.4s linear infinite; }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(250%); } }
  @media (prefers-reduced-motion: reduce) { .bar.indet { animation: none; inline-size: 100%; } }
  @media (forced-colors: active) { .bar { background: Highlight; } .track { border-color: ButtonText; } }
  .auto { display: grid; gap: var(--space-1); padding-block-start: var(--space-3); border-block-start: var(--border-width) solid var(--c-border-subtle); }
  .switch { display: inline-flex; align-items: center; gap: var(--space-3); min-block-size: var(--target-min); font-weight: var(--fw-semibold); justify-self: start; }
  /* Bryter: native avkrysning med eget utseende. Tilstand bæres av tommelposisjon og hake (ikke bare farge). */
  input[type="checkbox"] {
    appearance: none; position: relative; flex: none; margin: 0; inline-size: 2.5rem; block-size: 1.5rem; cursor: pointer;
    border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-pill); background: var(--c-surface-sunken);
  }
  input[type="checkbox"]::before {
    content: ""; position: absolute; inset-block: 0.1875rem; inset-inline-start: 0.1875rem; inline-size: 1rem; border-radius: var(--radius-pill);
    background: var(--c-text-muted); transition: transform var(--dur-fast);
  }
  input[type="checkbox"]:checked { background: var(--c-accent); border-color: var(--c-accent); }
  input[type="checkbox"]:checked::before { transform: translateX(1rem); background: var(--c-on-accent); }
  input[type="checkbox"]:disabled { opacity: 0.6; cursor: not-allowed; }
  input[type="checkbox"]:focus-visible { outline: var(--focus-width) solid var(--c-focus); outline-offset: var(--focus-offset); }
  @media (prefers-reduced-motion: reduce) { input[type="checkbox"]::before { transition: none; } }
  @media (forced-colors: active) {
    input[type="checkbox"] { border-color: ButtonText; background: Canvas; }
    input[type="checkbox"]::before { background: ButtonText; }
    input[type="checkbox"]:checked { background: Highlight; border-color: Highlight; }
    input[type="checkbox"]:checked::before { background: HighlightText; }
    input[type="checkbox"]:disabled { border-color: GrayText; }
  }
</style>
