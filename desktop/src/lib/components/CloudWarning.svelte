<!-- Advarsel om skysynket mappe (iCloud, OneDrive, Dropbox): forklarer at «Flytt til papirkurv» er slått av og hva brukeren kan gjøre.
     Gjenkjent etter mappenavn, derfor et forbehold i teksten. Samme tekst er beskrivelse (aria-describedby="cloud-why") for de sperrede knappene.
     Ingen live-region: banneret står der fra skjermen åpnes. Bygget på Banner (tone warn). -->
<script lang="ts">
  import { tr } from "../i18n";
  import Banner from "./Banner.svelte";

  interface Props {
    /** icloud | onedrive | dropbox (CloudCode). Ukjent verdi gir nøytral tekst. */
    provider: string;
  }
  let { provider }: Props = $props();
</script>

<Banner tone="warn" id="cloud-warning" titleId="cloud-title" title={$tr("cloud.title", { provider })}>
  <p id="cloud-why">{$tr("cloud.body")}</p>
  <p>{$tr("cloud.tip")}</p>
  <p class="caveat">{$tr("cloud.caveat")}</p>
</Banner>
