<!-- Innstillingskort: samler SettingRow-rader under en overskrift, rader skilt av tynne linjer. Bruker containerspørring,
     så radene stables når kortet (ikke vinduet) er smalt. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    heading?: string;
    headingLevel?: 2 | 3;
    /** Støttetekst under overskriften. */
    description?: string;
    children: Snippet;
  }
  let { heading, headingLevel = 2, description, children }: Props = $props();
  const uid = $props.id();
  const tag = $derived(`h${headingLevel}`);
</script>

<section class="sc" aria-labelledby={heading ? `${uid}-h` : undefined}>
  {#if heading}
    <div class="hd">
      <svelte:element this={tag} id="{uid}-h">{heading}</svelte:element>
      {#if description}<p>{description}</p>{/if}
    </div>
  {/if}
  <div class="rows">{@render children()}</div>
</section>

<style>
  .sc { container-type: inline-size; background: var(--c-surface); border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-lg); box-shadow: var(--shadow-card); }
  .hd { padding: var(--space-4) var(--space-5) var(--space-2); }
  .hd :global(h2), .hd :global(h3) { margin: 0; font-size: var(--fs-lg); line-height: var(--lh-tight); }
  .hd p { margin: var(--space-1) 0 0; color: var(--c-text-muted); font-size: var(--fs-sm); }
  .rows { padding-block: var(--space-1); }
</style>
