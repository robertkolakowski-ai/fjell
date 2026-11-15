<!-- Innstillinger: indeksering (plassholder i kontraktfasen; grenen «indeksering» eier filen, docs/v1/ROBUSTHET-KONTRAKT.md).
     Antall tråder (0 = automatisk), prioritet og «Pause på batteri». Lagres av kjernen; visningen følger det kjernen svarer.
     Tilstand og flyt i indexingFlow.ts (IndexingSettingsSession). -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { INDEXING_PRIORITIES, MAX_INDEX_THREADS, type IndexingPriority } from "../bridge/types-robust";
  import { useBridge } from "../context";
  import { Banner, Button, Card, SegmentedControl } from "../lib";
  import { tr, type MessageKey } from "../lib/i18n";
  import { IndexingSettingsSession, initialIndexingSettings, type IndexingSettingsView } from "./indexingFlow";

  interface Props {
    bridge?: CoreBridge;
  }
  let { bridge }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new IndexingSettingsSession(b, (s) => (view = s));
  let view: IndexingSettingsView = $state(initialIndexingSettings);

  const prioKey = { low: "indexing.priority.low", normal: "indexing.priority.normal" } as const satisfies Record<IndexingPriority, MessageKey>;
  const cur = $derived(view.settings);
  const prioOptions = $derived(INDEXING_PRIORITIES.map((p) => ({ value: p, label: $tr(prioKey[p]) })));
  const batteryOptions = $derived([
    { value: "on", label: $tr("indexing.battery.on") },
    { value: "off", label: $tr("indexing.battery.off") },
  ]);
  // 0 = automatisk, ellers 1..MAX_INDEX_THREADS.
  const threadChoices = Array.from({ length: MAX_INDEX_THREADS + 1 }, (_, n) => n);

  onMount(() => void session.load());
  onDestroy(() => session.dispose());
</script>

<Card heading={$tr("indexing.title")}>
  <div class="body">
    <p class="muted">{$tr("indexing.settings.description")}</p>
    {#if view.phase === "loading"}
      <p role="status">{$tr("common.loading")}</p>
    {:else if view.phase === "error" || !cur}
      <Banner tone="danger" role="alert">
        <p>{$tr("indexing.error")}</p>
        {#snippet actions()}<Button onclick={() => void session.load()}>{$tr("common.retry")}</Button>{/snippet}
      </Banner>
    {:else}
      <div class="block">
        <label class="lbl" for="idx-threads">{$tr("indexing.threads.label")}</label>
        <select id="idx-threads" value={String(cur.threads)} disabled={view.saving} onchange={(e) => void session.update({ threads: Number(e.currentTarget.value) })}>
          {#each threadChoices as n (n)}
            <option value={String(n)}>{n === 0 ? $tr("indexing.threads.auto") : n}</option>
          {/each}
        </select>
      </div>
      <div class="block">
        <p id="idx-p" class="lbl">{$tr("indexing.priority.label")}</p>
        <SegmentedControl name="idx-priority" options={prioOptions} value={cur.priority} disabled={view.saving} ariaLabelledby="idx-p"
          onchange={(v) => void session.update({ priority: v as IndexingPriority })} />
      </div>
      <div class="block">
        <p id="idx-b" class="lbl">{$tr("indexing.battery.label")}</p>
        <SegmentedControl name="idx-battery" options={batteryOptions} value={cur.pauseOnBattery ? "on" : "off"} disabled={view.saving} ariaLabelledby="idx-b"
          onchange={(v) => void session.update({ pauseOnBattery: v === "on" })} />
      </div>
      {#if view.error}
        <Banner tone="danger" role="alert"><p>{$tr("indexing.settings.error")}</p></Banner>
      {/if}
    {/if}
  </div>
</Card>

<style>
  .body { display: grid; gap: var(--space-4); }
  .body p { margin: 0; }
  .block { display: grid; gap: var(--space-2); justify-items: start; }
  .lbl { font-weight: var(--fw-medium); }
  .muted { color: var(--c-text-muted); font-size: var(--fs-sm); }
  select { min-block-size: var(--target-min); }
</style>
