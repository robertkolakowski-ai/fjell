<!-- Felles aria-live="polite"-region (WCAG 4.1.3). Én per skjermflate. Bytter tekst uten fokusflytting.
     delay > 0 demper hyppige endringer (bunnlinjesum: 300 ms). Lik melding to ganger på rad annonseres på nytt. -->
<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { debounce } from "../debounce.ts";

  interface Props {
    message: string;
    delay?: number;
  }
  let { message, delay = 0 }: Props = $props();

  let shown = $state("");
  let flip = $state(false);

  function publish(m: string) {
    shown = m;
    flip = !flip; // nytt DOM-element tvinger gjenannonsering av identisk tekst (leser flip: kalles i $effect, så untrack nedenfor)
  }
  const debounced = debounce(publish, () => delay);

  $effect(() => {
    const m = message;
    // untrack: publish leser og skriver `flip`; uten dette avhenger effekten av egen skriving og går i uendelig løkke.
    untrack(() => {
      if (m === "") publish("");
      else if (delay > 0) debounced(m);
      else publish(m);
    });
  });
  onDestroy(() => debounced.cancel());
</script>

<div class="sr-only" role="status" aria-live="polite" aria-atomic="true">
  {#key flip}<span>{shown}</span>{/key}
</div>
