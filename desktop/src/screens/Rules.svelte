<!-- Regler. Monteringspunkt for rules-grenen: fyll denne filen. Regler foreslår bare; slett kan angres (restoreRule).
     Liste, ny regel med forhåndsvisning (previewRule), av/på (toggleRule) via `useBridge()`. -->
<script lang="ts">
  import { PageHeader } from "../lib";
  import { tr } from "../lib/i18n";
</script>

<section class="rules">
  <PageHeader title={$tr("rules.title.label")} />
  <p class="todo">{$tr("placeholder.todo")}</p>
</section>

<style>
  .rules { max-inline-size: var(--w-content); }
  .todo { margin: 0; color: var(--c-text-muted); }
</style>
