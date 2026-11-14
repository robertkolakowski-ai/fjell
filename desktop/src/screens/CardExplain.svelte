<!-- Forklaring av et forslag (begrunnelsesmerke med tall). Monteringspunkt for series- og rules-grenene: fyll denne filen.
     Brukes i Kortmodus for serier og regler. Appen foreslår, den dømmer ikke: forklaringen sier hvorfor, ikke hva brukeren bør gjøre. -->
<script lang="ts">
  import type { Detail, ReasonCode } from "../bridge/types";
  import { tr } from "../lib/i18n";

  interface Props {
    reason: ReasonCode;
    detail: Detail;
  }
  let { reason, detail }: Props = $props();
</script>

<div class="explain" data-reason={reason} data-detail={detail.kind}>
  <p class="title">{$tr("card.explain.title")}</p>
  <p class="todo">{$tr("placeholder.todo")}</p>
</div>

<style>
  .explain { margin-block-start: var(--space-2); }
  .explain p { margin: 0; }
  .title { font-size: var(--fs-sm); font-weight: var(--fw-semibold); }
  .todo { color: var(--c-text-muted); font-size: var(--fs-xs); }
</style>
