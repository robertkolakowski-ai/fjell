<!-- Kort: flate med valgfri overskrift. Innholdet styrer layouten; ingen faste bredder på tekst. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    heading?: string;
    headingLevel?: 2 | 3 | 4;
    children: Snippet;
  }
  let { heading, headingLevel = 2, children }: Props = $props();
  const tag = $derived(`h${headingLevel}`);
</script>

<section class="card" aria-label={heading}>
  {#if heading}
    <svelte:element this={tag} class="title">{heading}</svelte:element>
  {/if}
  {@render children()}
</section>

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
