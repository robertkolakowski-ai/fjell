<!-- Innstillinger: «Hvor mange forslag». Én segmentvelger (lav, normal, høy) per forslagstype der sensitivitet flytter noe
     (like bilder, uskarpe, serier), med forklaring, og terskelverdiene som skrivebeskyttet info (ingen felt: de justeres ikke her).
     Appen foreslår bare; valget gjelder nye forslag. Tilstand i sensitivityFlow.ts. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import type { KindCode } from "../bridge/types";
  import type { SensitivityCode } from "../bridge/types-v1";
  import { useBridge } from "../context";
  import { Banner, Button, Card, LiveRegion, SegmentedControl } from "../lib";
  import { fmt, getIntlLocale, tr } from "../lib/i18n";
  import { initialSensitivity, plainNumber, SENSITIVE_KINDS, SENSITIVITY_CODES, SensitivitySession, type SensitivityState } from "./sensitivityFlow";

  interface Props {
    bridge?: CoreBridge;
  }
  let { bridge }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new SensitivitySession(b, (s) => (view = s));

  let view: SensitivityState = $state(initialSensitivity);

  const kindKey = { visual_dup: "home.cat.visual_dup", blurry: "home.cat.blurry", series: "home.cat.series" } as const;
  const helpKey = { visual_dup: "settings.sensitivity.help.visual_dup", blurry: "settings.sensitivity.help.blurry", series: "settings.sensitivity.help.series" } as const;
  const levelKey = { low: "settings.sensitivity.low", normal: "settings.sensitivity.normal", high: "settings.sensitivity.high" } as const satisfies Record<SensitivityCode, string>;
  const options = $derived(SENSITIVITY_CODES.map((c) => ({ value: c, label: $tr(levelKey[c]) })));
  const announce = $derived(view.saved ? $tr("settings.saved") : view.saveFailed ? $tr("settings.error") : "");

  /** Terskler som ferdige tekster. Regnes om når språket byttes ($tr endres). */
  const thresholds = $derived.by(() => {
    const th = view.thresholds;
    if (!th) return [];
    const intl = getIntlLocale();
    return [
      { kind: "visual_dup" as KindCode, text: $tr("settings.sensitivity.th.hamming", { value: plainNumber(th.visualMaxHamming, intl) }) },
      { kind: "blurry" as KindCode, text: $tr("settings.sensitivity.th.blur", { value: plainNumber(th.blurMaxScore, intl) }) },
      { kind: "series" as KindCode, text: $tr("settings.sensitivity.th.gap", { value: plainNumber(th.seriesGapSecs, intl) }) },
      { kind: "series" as KindCode, text: $tr("settings.sensitivity.th.eyes", { value: fmt.percent(th.closedEyesMinConfidence) }) },
    ];
  });

  onMount(() => void session.load());
  onDestroy(() => session.dispose());
</script>

<Card heading={$tr("settings.sensitivity.title")}>
  <div class="body">
    <LiveRegion message={announce} />
    <p class="muted">{$tr("settings.sensitivity.note")}</p>

    {#if view.phase === "loading"}
      <p role="status">{$tr("common.loading")}</p>
    {:else if view.phase === "error"}
      <Banner tone="danger" role="alert"><p>{$tr("settings.sensitivity.error")}</p>
        {#snippet actions()}<Button onclick={() => void session.load()}>{$tr("common.retry")}</Button>{/snippet}
      </Banner>
    {:else if view.settings}
      {@const settings = view.settings}
      {#each SENSITIVE_KINDS as kind (kind)}
        <div class="kind">
          <p class="kname" id={`sens-${kind}-l`}>{$tr(kindKey[kind as keyof typeof kindKey])}</p>
          <SegmentedControl name={`sens-${kind}`} options={options} value={settings.sensitivity[kind]} disabled={view.saving}
            ariaLabelledby={`sens-${kind}-l`} ariaDescribedby={`sens-${kind}-h`}
            onchange={(v) => void session.choose(kind, v as SensitivityCode)} />
          <p class="muted" id={`sens-${kind}-h`}>{$tr(helpKey[kind as keyof typeof helpKey])}</p>
        </div>
      {/each}
      <p class="muted">{$tr("settings.sensitivity.none")}</p>

      {#if thresholds.length > 0}
        <section class="th" aria-labelledby="sens-th">
          <h3 id="sens-th">{$tr("settings.sensitivity.th.heading")}</h3>
          <ul>
            {#each thresholds as x (x.text)}<li>{x.text}</li>{/each}
          </ul>
        </section>
      {/if}
    {/if}
  </div>
</Card>

<style>
  .body { display: grid; gap: var(--space-3); }
  .body p { margin: 0; }
  .muted { color: var(--c-text-muted); font-size: var(--fs-sm); }
  .kind { display: grid; gap: var(--space-1); justify-items: start; padding-block-start: var(--space-3); border-block-start: var(--border-width) solid var(--c-border-subtle); }
  .kname { font-weight: var(--fw-semibold); }
  .th { padding-block-start: var(--space-3); border-block-start: var(--border-width) solid var(--c-border-subtle); }
  .th h3 { margin: 0 0 var(--space-1); font-size: var(--fs-sm); font-weight: var(--fw-semibold); }
  .th ul { margin: 0; padding-inline-start: var(--space-5); color: var(--c-text-muted); font-size: var(--fs-sm); font-variant-numeric: var(--num); }
</style>
