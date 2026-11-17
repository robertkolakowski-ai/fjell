<!-- Indekseringsstatus: rolig fremdrift med Pause/Fortsett-knapper (docs/v1/INDEKSERING.md). Indekseringen går i bakgrunnen og
     blokkerer ikke gjennomgang: raden sier det, og forslagene oppdateres underveis. Tilstandene er koder (kjører, pauset, venter på
     strøm, frakoblet disk); ingen skremmende tekst, og «frakoblet disk» sier uttrykkelig at ingenting er slettet.
     Uten `controls` (Hjem, som har skannlinjen sin) vises bare det den ikke dekker: batteri, frakoblet disk og gjenstående tid.
     Med `controls` (Innstillinger, bunnlinje) vises tilstand, fremdriftslinje, tall, gjenstående tid og knappene.
     Tilstandsendringer kunngjøres i en aria-live-region (en melding per endring, ikke per polling). Flyt i indexingFlow.ts. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { useBridge } from "../context";
  import { Button, LiveRegion } from "../lib";
  import { fmt, tr } from "../lib/i18n";
  import { etaMinutes, hintKey, IndexingSession, initialIndexing, isHalted, remaining, stateKey, type IndexingView } from "./indexingFlow";
  import { robustErrorKey } from "./robustErrors";

  interface Props {
    bridge?: CoreBridge;
    /** Pollingintervall (ms). */
    pollMs?: number;
    /** Vis fremdriftslinje, tall og knappene Pause, Fortsett og Avbryt. Standard av: Hjem har dem i skannlinjen. */
    controls?: boolean;
  }
  let { bridge, pollMs = 2000, controls = false }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new IndexingSession(b, (s) => (view = s));
  let view: IndexingView = $state(initialIndexing);

  const st = $derived(view.status);
  const minutes = $derived(etaMinutes(st));
  const halted = $derived(isHalted(st.state));
  const hint = $derived(hintKey(st.state));
  // Kompakt visning (Hjem): bare det skannlinjen ikke sier. Full visning: alt så lenge det ikke er ledig.
  const show = $derived(view.loaded && (controls ? st.state !== "idle" : halted || (st.state === "running" && minutes !== null)));
  const announce = $derived(view.announce ? $tr(view.announce) : "");

  onMount(() => session.start(pollMs));
  onDestroy(() => session.dispose());
</script>

{#if show}
  <div class="idx" role="group" aria-label={$tr(controls ? "indexing.progress.label" : "indexing.status.label")}>
    <p class="line">
      {$tr(stateKey(st.state))}
      {#if st.state === "running" && minutes !== null}<span class="eta">{$tr("indexing.eta", { minutes })}</span>{/if}
    </p>
    {#if controls && st.total > 0}
      <div class="bar">
        <progress aria-label={$tr("indexing.progress.label")} max={st.total} value={Math.min(st.done, st.total)}></progress>
        <p class="num">
          {$tr("indexing.progress", { done: fmt.number(st.done), total: fmt.number(st.total) })}
          <span class="sep" aria-hidden="true">·</span>
          {$tr("indexing.remaining", { remaining: fmt.number(remaining(st)) })}
        </p>
      </div>
    {/if}
    {#if hint}<p class="hint">{$tr(hint)}</p>{/if}
    <p class="hint">{$tr("indexing.note.live")}</p>
    {#if controls}
      <div class="ctl">
        {#if st.state === "running"}
          <Button size="sm" disabled={view.busy} onclick={() => void session.pause()}>{$tr("indexing.pause")}</Button>
        {:else}
          <Button size="sm" disabled={view.busy} onclick={() => void session.resume()}>{$tr("indexing.resume")}</Button>
        {/if}
        <Button size="sm" disabled={view.busy} onclick={() => void session.cancel()}>{$tr("indexing.cancel")}</Button>
      </div>
    {/if}
    {#if view.error}<p role="alert">{$tr(robustErrorKey(view.error))}</p>{/if}
  </div>
{/if}
<LiveRegion message={announce} />

<style>
  .idx { display: grid; gap: var(--space-2); justify-items: start; color: var(--c-text-muted); font-size: var(--fs-sm); }
  .idx p { margin: 0; }
  .line { color: var(--c-text); }
  .eta { margin-inline-start: var(--space-2); color: var(--c-text-muted); }
  .bar { display: grid; gap: var(--space-1); inline-size: 100%; max-inline-size: 24rem; }
  progress { inline-size: 100%; block-size: var(--space-2); accent-color: var(--c-accent); }
  .num { font-variant-numeric: var(--num); }
  .sep { margin-inline: var(--space-1); }
  .hint { max-inline-size: 60ch; }
  .ctl { display: flex; flex-wrap: wrap; gap: var(--space-2); }
</style>
