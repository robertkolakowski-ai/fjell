<!-- Forklaring av et forslag: hvorfor appen foreslår dette (begrunnelsesmerket i klartekst) og tallene bak det
     (prosent, antall, serieposisjon, regel, person). Brukes i Kortmodus, Gruppe og Serie-sammenligning.
     Appen foreslår, den dømmer ikke: teksten sier hvorfor, aldri hva brukeren bør gjøre. Tilstand i explain.ts. -->
<script lang="ts">
  import type { Detail, ReasonCode } from "../bridge/types";
  import { tr } from "../lib/i18n";
  import { explainBody, explainFacts } from "./explain";
  import type { RuleNames } from "./reason";

  interface Props {
    reason: ReasonCode;
    detail: Detail;
    /** Til uskarphetsprosent når `detail` ikke har tall. */
    confidence?: number;
    /** Regelnavn per id (brukerens egne navn; forlater aldri enheten). */
    ruleNames?: RuleNames;
  }
  let { reason, detail, confidence, ruleNames }: Props = $props();
  const input = $derived({ reason, detail, confidence, ruleNames });
  const body = $derived(explainBody(input, $tr));
  const facts = $derived(explainFacts(input, $tr));
</script>

<div class="explain" data-reason={reason} data-detail={detail.kind}>
  <p class="title">{$tr("card.explain.title")}</p>
  <p class="body">{body}</p>
  {#if facts.length > 0}
    <dl class="facts">
      {#each facts as f (f.code)}
        <dt>{f.label}</dt><dd data-fact={f.code}>{f.value}</dd>
      {/each}
    </dl>
  {/if}
  <p class="note">{$tr("card.explain.note")}</p>
</div>

<style>
  .explain { margin-block-start: var(--space-2); min-inline-size: 0; }
  .explain p { margin: 0; overflow-wrap: anywhere; }
  .title { font-size: var(--fs-sm); font-weight: var(--fw-semibold); }
  .body { font-size: var(--fs-sm); }
  .facts { display: flex; flex-wrap: wrap; gap: var(--space-1) var(--space-3); margin: var(--space-1) 0 0; font-size: var(--fs-sm); }
  .facts dt { color: var(--c-text-muted); }
  .facts dd { margin: 0; font-weight: var(--fw-semibold); font-variant-numeric: var(--num); }
  .note { margin-block-start: var(--space-1); color: var(--c-text-muted); font-size: var(--fs-xs); }
</style>
