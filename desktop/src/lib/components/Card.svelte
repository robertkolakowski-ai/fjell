<!-- Kort: flate med valgfri overskrift. Innholdet styrer layouten; ingen faste bredder på tekst. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    heading?: string;
    headingLevel?: 2 | 3 | 4;
    children: Snippet;
  }
  let { heading, headingLevel = 2, children }: Props = $props();
  const uid = $props.id();
  const tag = $derived(`h${headingLevel}`);
</script>

<!-- Gruppe, ikke region: et landemerke per kort gir støy i Narrator/NVDA-landemerkelisten, og aria-label som gjentar
     overskriften leses to ganger. Navnet kommer fra selve overskriften. -->
<div class="card" role={heading ? "group" : undefined} aria-labelledby={heading ? `${uid}-t` : undefined}>
  {#if heading}
    <svelte:element this={tag} id="{uid}-t" class="title">{heading}</svelte:element>
  {/if}
  {@render children()}
</div>

<style>
  .card {
    background: var(--c-surface);
    color: var(--c-text);
    border: var(--border-width) solid var(--c-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-sm);
    padding: var(--space-4) var(--space-5);
  }
  .title {
    margin: 0 0 var(--space-2);
    font-size: var(--fs-lg);
    line-height: var(--lh-tight);
  }
</style>
