<!-- Oversikt (dashboard) på Hjem. Monteringspunkt for dashboard-grenen: fyll denne filen, ikke Home.svelte.
     Bruk `useBridge().getDashboard()`; kjernen sender koder og tall, teksten kommer fra i18n (`dashboard.*`). -->
<script lang="ts">
  import { Card } from "../lib";
  import { tr } from "../lib/i18n";
</script>

<Card heading={$tr("dashboard.title.label")} headingLevel={2}>
  <p class="todo">{$tr("placeholder.todo")}</p>
</Card>

<style>
  .todo { margin: 0; color: var(--c-text-muted); font-size: var(--fs-sm); }
</style>
