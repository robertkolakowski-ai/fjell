<!-- Innstillinger: seksjon «SettingsFaces». Monteringspunkt: fyll denne filen, ikke Settings.svelte.
     Ansiktsindeks (av som standard), slett indeksen med bekreftelse. getFaceIndex/setFaceIndex/wipeFaceIndex. Bruk `useBridge()`. -->
<script lang="ts">
  import { Card } from "../lib";
  import { tr } from "../lib/i18n";
</script>

<Card heading={$tr("settings.faces.title")}>
  <p class="todo">{$tr("placeholder.todo")}</p>
</Card>

<style>
  .todo { margin: 0; color: var(--c-text-muted); font-size: var(--fs-sm); }
</style>
