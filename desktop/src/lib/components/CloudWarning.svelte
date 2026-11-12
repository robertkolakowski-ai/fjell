<!-- Advarsel om skysynket mappe (iCloud, OneDrive, Dropbox): forklarer at «Flytt til papirkurv» er slått av og hva brukeren kan gjøre.
     Gjenkjent etter mappenavn, derfor et forbehold i teksten. Samme tekst er beskrivelse (aria-describedby="cloud-why") for de sperrede knappene.
     Ingen live-region: banneret står der fra skjermen åpnes. -->
<script lang="ts">
  import { tr } from "../i18n";

  interface Props {
    /** icloud | onedrive | dropbox (CloudCode). Ukjent verdi gir nøytral tekst. */
    provider: string;
  }
  let { provider }: Props = $props();
</script>

<div class="cloud" id="cloud-warning" role="group" aria-labelledby="cloud-title">
  <h2 id="cloud-title">{$tr("cloud.title", { provider })}</h2>
  <p id="cloud-why">{$tr("cloud.body")}</p>
  <p>{$tr("cloud.tip")}</p>
  <p class="caveat">{$tr("cloud.caveat")}</p>
</div>

<style>
  .cloud { margin-block: 0 var(--space-4); padding: var(--space-3) var(--space-4); border: 3px solid var(--c-border-strong); border-radius: var(--radius-md); background: var(--c-surface); color: var(--c-text); max-inline-size: 60rem; }
  h2 { margin-block: 0 var(--space-2); font-size: var(--fs-lg); line-height: var(--lh-tight); }
  p { margin-block: 0 var(--space-2); overflow-wrap: anywhere; }
  .caveat { margin-block-end: 0; color: var(--c-text-muted); }
</style>
