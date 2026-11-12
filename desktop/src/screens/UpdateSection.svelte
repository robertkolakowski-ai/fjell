<!-- Oppdateringer (M9): versjon, «Se etter oppdatering», status i live region, «Installer og start på nytt» med fremdrift,
     bryter «Sjekk automatisk ved oppstart» (standard av). Sjekk skjer bare på knappetrykk eller når auto er på (App.svelte).
     Aldri automatisk installasjon, og ingen installasjon mens skanning pågår (aria-disabled + forklaring, knappen beholder fokus).
     Tilstand i updateState.ts, flyt i updateFlow.ts. Notatene fra utgivelsen vises som ren tekst (aldri som HTML). -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { useBridge } from "../context";
  import { Button, LiveRegion } from "../lib";
  import { fmt, tr } from "../lib/i18n";
  import { UpdateSession } from "./updateFlow";
  import { initialUpdate, installBlocked, progressPercent, type UpdateView } from "./updateState";

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

  const found = $derived(view.found);
  const blocked = $derived(installBlocked(view));
  const busy = $derived(view.status === "checking" || view.status === "installing" || view.status === "restarting");
  const showInstall = $derived(found !== null && (view.status === "available" || view.status === "installing" || view.status === "restarting" || view.status === "installFailed"));
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
      case "installFailed": return $tr("update.error.install");
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

  // Mens en ny versjon venter: les skannstatus på nytt, så knappen åpnes når skannet er pauset, avbrutt eller ferdig.
  $effect(() => {
    clearInterval(timer);
    if (view.status === "available" || view.status === "installFailed") timer = setInterval(() => void session.refreshScan(), pollMs);
    return () => clearInterval(timer);
  });
  onMount(() => void session.load());
  onDestroy(() => { clearInterval(timer); session.dispose(); });
</script>

<section class="update" aria-labelledby="update-h">
  <h2 id="update-h">{$tr("update.title")}</h2>
  <LiveRegion message={message} />

  <p class="ver">{view.version === null ? "" : $tr("update.version", { version: view.version })}</p>

  <div class="row">
    <Button ariaDisabled={busy} onclick={check}>{$tr("update.check")}</Button>
    {#if showInstall}
      <Button variant="primary" ariaDisabled={busy || blocked} ariaDescribedby={blocked ? "update-blocked" : undefined} onclick={install}>{$tr("update.install")}</Button>
    {/if}
  </div>

  {#if blocked}
    <p id="update-blocked" class="muted">{$tr("update.blocked")}</p>
  {/if}

  {#if message !== ""}
    <p class="status" class:err={view.status === "error" || view.status === "installFailed"}>{message}</p>
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

  <label class="switch">
    <input
      type="checkbox" role="switch" checked={view.auto} disabled={view.autoSaving} aria-describedby="update-auto-note"
      onchange={(e) => void session.setAuto(e.currentTarget.checked)}
    />
    {$tr("update.auto.label")}
  </label>
  <p id="update-auto-note" class="muted">{$tr("update.auto.note")}</p>
  {#if view.autoFailed}<p role="alert">{$tr("update.auto.error")}</p>{/if}
</section>

<style>
  .update { margin: 0 0 var(--space-5); display: grid; gap: var(--space-2); }
  h2 { margin-block: 0 var(--space-1); font-size: var(--fs-lg); }
  h3 { margin: 0; font-size: var(--fs-md); }
  p { margin: 0; }
  .muted { color: var(--c-text-muted); }
  .row { display: flex; flex-wrap: wrap; gap: var(--space-3); }
  .status.err { font-weight: var(--fw-semibold); }
  .text { white-space: pre-line; overflow-wrap: anywhere; }
  .notes { display: grid; gap: var(--space-1); padding: var(--space-3) var(--space-4); border: var(--border-width) solid var(--c-border); border-radius: var(--radius-md); background: var(--c-surface); }
  .progress { display: grid; gap: var(--space-1); }
  .track { block-size: 0.75rem; overflow: hidden; border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-md); background: var(--c-surface-sunken); }
  .bar { block-size: 100%; background: var(--c-text); }
  .bar.indet { inline-size: 40%; animation: slide 1.4s linear infinite; }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(250%); } }
  @media (prefers-reduced-motion: reduce) { .bar.indet { animation: none; inline-size: 100%; } }
  @media (forced-colors: active) { .bar { background: Highlight; } .track { border-color: ButtonText; } }
  .switch { display: flex; align-items: center; gap: var(--space-2); min-block-size: var(--target-min); margin-block-start: var(--space-2); }
  input[type="checkbox"] { inline-size: 1.25rem; block-size: 1.25rem; flex: none; }
</style>
