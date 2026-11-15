<!-- Indekseringsstatus (plassholder i kontraktfasen; grenen «indeksering» eier filen, docs/v1/ROBUSTHET-KONTRAKT.md).
     Hjem har allerede skannlinjen (fremdrift, pause, fortsett, avbryt). Her vises det den ikke dekker: gjenstående tid mens det går,
     og at indekseringen står stille fordi PC-en går på batteri. Ingenting vises når det er ledig. `controls` viser egne knapper
     (for monteringssteder uten skannlinjen). Tilstand og flyt i indexingFlow.ts. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { useBridge } from "../context";
  import { Button, LiveRegion } from "../lib";
  import { fmt, tr } from "../lib/i18n";
  import { etaMinutes, IndexingSession, initialIndexing, type IndexingView } from "./indexingFlow";
  import { robustErrorKey } from "./robustErrors";

  interface Props {
    bridge?: CoreBridge;
    /** Pollingintervall (ms). */
    pollMs?: number;
    /** Vis pause, fortsett og avbryt. Standard av: Hjem har dem i skannlinjen. */
    controls?: boolean;
  }
  let { bridge, pollMs = 2000, controls = false }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new IndexingSession(b, (s) => (view = s));
  let view: IndexingView = $state(initialIndexing);

  const st = $derived(view.status);
  const minutes = $derived(etaMinutes(st));
  const show = $derived(view.loaded && (st.state === "paused_on_battery" || (st.state === "running" && minutes !== null) || (controls && st.state !== "idle")));
  const announce = $derived(st.state === "paused_on_battery" ? $tr("indexing.state.paused_on_battery") : "");

  onMount(() => session.start(pollMs));
  onDestroy(() => session.dispose());
</script>

{#if show}
  <div class="idx" role="group" aria-label={$tr("indexing.status.label")}>
    <p class="line">
      {$tr(st.state === "running" ? "indexing.state.running" : st.state === "paused" ? "indexing.state.paused" : st.state === "paused_on_battery" ? "indexing.state.paused_on_battery" : "indexing.state.idle")}
      {#if controls && st.total > 0}{$tr("indexing.progress", { done: fmt.number(st.done), total: fmt.number(st.total) })}{/if}
      {#if st.state === "running" && minutes !== null}{$tr("indexing.eta", { minutes })}{/if}
    </p>
    {#if controls}
      <div class="ctl">
        {#if st.state === "running"}
          <Button size="sm" disabled={view.busy} onclick={() => void session.pause()}>{$tr("indexing.pause")}</Button>
        {:else if st.state !== "idle"}
          <Button size="sm" disabled={view.busy} onclick={() => void session.resume()}>{$tr("indexing.resume")}</Button>
        {/if}
        <Button size="sm" disabled={view.busy} onclick={() => void session.cancel()}>{$tr("indexing.cancel")}</Button>
      </div>
    {/if}
    {#if view.error}<p role="alert">{$tr(robustErrorKey(view.error))}</p>{/if}
  </div>
  <LiveRegion message={announce} />
{/if}

<style>
  .idx { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-3); color: var(--c-text-muted); font-size: var(--fs-sm); }
  .idx p { margin: 0; }
  .ctl { display: flex; gap: var(--space-2); }
</style>
