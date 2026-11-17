<!-- Innstillinger: diagnose (docs/v1/DIAGNOSE.md). Brukeren ser hva som er og ikke er med, og rapporten (tall og koder,
     aldri stier, filnavn eller bilder) før noe lagres. Filen lagres i appens diagnosemappe; appen sender den aldri noe sted.
     Personvernnotatet bruker nøklene i privacy.*. Tilstand og flyt i diagFlow.ts. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { useBridge } from "../context";
  import { Banner, Button, Card, LiveRegion } from "../lib";
  import { fmt, tr, type MessageKey } from "../lib/i18n";
  import { DiagSession, initialDiag, type DiagView } from "./diagFlow";
  import { robustErrorKey } from "./robustErrors";

  interface Props {
    bridge?: CoreBridge;
  }
  let { bridge }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new DiagSession(b, (s) => (view = s));
  let view: DiagView = $state(initialDiag);

  const metricKey = {
    schema_version: "diag.metric.schema_version", assets_active: "diag.metric.assets_active", assets_trashed: "diag.metric.assets_trashed",
    groups: "diag.metric.groups", suggestions: "diag.metric.suggestions", rules: "diag.metric.rules",
    protections: "diag.metric.protections", backups: "diag.metric.backups",
  } as const satisfies Record<string, MessageKey>;
  const announce = $derived(view.exported ? $tr("diag.export.done") : "");

  onMount(() => void session.load());
  onDestroy(() => session.dispose());
</script>

<Card heading={$tr("diag.title")}>
  <div class="body">
    <p class="muted">{$tr("diag.description")}</p>
    <p class="muted">{$tr("privacy.local")} {$tr("privacy.diag.note")}</p>
    <div class="scope">
      <p><strong>{$tr("diag.included.title")}</strong> {$tr("diag.included.body")}</p>
      <p><strong>{$tr("diag.excluded.title")}</strong> {$tr("diag.excluded.body")}</p>
      <p class="muted">{$tr("diag.folders.off")}</p>
    </div>
    {#if view.phase === "loading"}
      <p role="status">{$tr("common.loading")}</p>
    {:else if view.phase === "error" || !view.report}
      <Banner tone="danger" role="alert">
        <p>{$tr("diag.load.error")}</p>
        {#snippet actions()}<Button onclick={() => void session.load()}>{$tr("common.retry")}</Button>{/snippet}
      </Banner>
    {:else}
      {#if view.report.metrics.length === 0}
        <p class="muted">{$tr("diag.metrics.empty")}</p>
      {:else}
        <dl class="metrics">
          {#each view.report.metrics as m (m.code)}
            <dt>{$tr(metricKey[m.code])}</dt>
            <dd>{fmt.number(m.value)}</dd>
          {/each}
        </dl>
      {/if}
      <label class="lbl" for="diag-text">{$tr("diag.preview.label")}</label>
      <textarea id="diag-text" readonly rows="10" value={view.report.text}></textarea>
      <p class="muted">{$tr("diag.preview.hint")}</p>
      <div><Button disabled={view.busy} onclick={() => void session.exportReport()}>{$tr("diag.export.button")}</Button></div>
      {#if view.exported}
        <Banner tone="ok" role="status">
          <p>{$tr("diag.export.done")}</p>
          {#if view.savedPath}
            <p>{$tr("diag.saved.path")} <code class="path">{view.savedPath}</code></p>
          {/if}
          <p>{$tr("diag.saved.hint")}</p>
        </Banner>
      {/if}
      {#if view.error}
        <Banner tone="danger" role="alert"><p>{$tr(robustErrorKey(view.error))}</p></Banner>
      {/if}
    {/if}
  </div>
</Card>
<LiveRegion message={announce} />

<style>
  .body { display: grid; gap: var(--space-3); }
  .body p { margin: 0; }
  .muted { color: var(--c-text-muted); font-size: var(--fs-sm); }
  .scope { display: grid; gap: var(--space-2); }
  .path { overflow-wrap: anywhere; font-family: var(--font-mono, monospace); }
  .lbl { font-weight: var(--fw-medium); }
  .metrics { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--space-1) var(--space-4); margin: 0; }
  .metrics dd { margin: 0; text-align: end; font-variant-numeric: tabular-nums; }
  .metrics dt { margin: 0; }
  textarea { inline-size: 100%; font-family: var(--font-mono, monospace); font-size: var(--fs-sm); color: var(--c-text); background: var(--c-surface-sunken); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-md); padding: var(--space-2); box-sizing: border-box; }
</style>
