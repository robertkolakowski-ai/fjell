<!-- Sammenlign serie (`#/series/:id`, underside av Gjennomgang). Monteringspunkt for series-grenen: fyll denne filen.
     `useBridge().compareSeries(id)` gir medlemmene med foreslått behold-kandidat først; `setKeeper(id, asset)` bytter. -->
<script lang="ts">
  import { PageHeader } from "../lib";
  import { tr } from "../lib/i18n";
  import { formatHash } from "../router";

  interface Props { id: number }
  let { id }: Props = $props();
</script>

<section class="series" data-series={id}>
  <PageHeader title={$tr("series.compare.title")} back={{ href: formatHash({ name: "review", kind: "series" }), label: $tr("review.back") }} />
  <p class="todo">{$tr("placeholder.todo")}</p>
</section>

<style>
  .series { max-inline-size: var(--w-wide); }
  .todo { margin: 0; color: var(--c-text-muted); }
</style>
