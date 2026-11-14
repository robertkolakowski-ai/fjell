<!-- Tom tilstand (V2 2.7): kotelinje-illustrasjon, én setning i lg, én i sm dempet, høyst én handling.
     Illustrasjonen er dekorativ. Tittelen er en overskrift (nivå styres av `headingLevel`; bruk 2 under sideoverskriften). -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import Contours, { type ContourMotif } from "./Contours.svelte";

  interface Props {
    /** topp = ingen funn, vann = tom papirkurv, sti = ingen mappe valgt. */
    motif?: ContourMotif;
    title: string;
    description?: string;
    headingLevel?: 2 | 3;
    /** Høyst én knapp. */
    action?: Snippet;
  }
  let { motif = "topp", title, description, headingLevel = 2, action }: Props = $props();
  const tag = $derived(`h${headingLevel}`);
</script>

<div class="empty">
  <Contours {motif} />
  <svelte:element this={tag} class="title">{title}</svelte:element>
  {#if description}<p class="desc">{description}</p>{/if}
  {#if action}<div class="act">{@render action()}</div>{/if}
</div>

<style>
  .empty { display: flex; flex-direction: column; align-items: center; gap: var(--space-2); padding: var(--space-6) var(--space-4); text-align: center; animation: rise var(--dur-3) var(--ease-out) backwards; }
  .title { margin: var(--space-3) 0 0; font-size: var(--fs-lg); font-weight: var(--fw-semibold); line-height: var(--lh-heading); max-inline-size: 36ch; }
  .desc { margin: 0; color: var(--c-text-muted); font-size: var(--fs-sm); max-inline-size: 46ch; }
  .act { margin-block-start: var(--space-3); }
  @keyframes rise { from { opacity: 0; transform: translateY(0.5rem); } to { opacity: 1; transform: none; } }
</style>
