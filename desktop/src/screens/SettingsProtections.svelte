<!-- Innstillinger: seksjon «SettingsProtections». Monteringspunkt: fyll denne filen, ikke Settings.svelte.
     Liste over beskyttelser med Fjern (angres). listProtections/protect. Bruk `useBridge()`. -->
<script lang="ts">
  import { Card } from "../lib";
  import { tr } from "../lib/i18n";
</script>

<Card heading={$tr("settings.protections.title")}>
  <p class="todo">{$tr("placeholder.todo")}</p>
</Card>

<style>
  .todo { margin: 0; color: var(--c-text-muted); font-size: var(--fs-sm); }
</style>
