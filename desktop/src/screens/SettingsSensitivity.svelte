<!-- Innstillinger: seksjon «SettingsSensitivity». Monteringspunkt: fyll denne filen, ikke Settings.svelte.
     Sensitivitet per forslagstype (low/normal/high) og terskler. getSuggestionSettings/setSuggestionSettings/getThresholds. Bruk `useBridge()`. -->
<script lang="ts">
  import { Card } from "../lib";
  import { tr } from "../lib/i18n";
</script>

<Card heading={$tr("settings.sensitivity.title")}>
  <p class="todo">{$tr("placeholder.todo")}</p>
</Card>

<style>
  .todo { margin: 0; color: var(--c-text-muted); font-size: var(--fs-sm); }
</style>
