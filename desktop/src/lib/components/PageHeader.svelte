<!-- Sideoverskrift: h1 (får fokus ved rutebytte, derfor tabindex=-1), valgt undertittel/meta og handlinger til høyre.
     Bryter til to rader når det ikke er plass (norsk +30 %). Tilbakelenke vises over tittelen. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    /** Valgfri tekst etter tittelen i vanlig vekt, f.eks. kategorinavn. */
    sub?: string;
    /** Valgfri støttetekst under tittelen. */
    description?: string;
    /** Tilbakelenke over tittelen. */
    back?: { href: string; label: string };
    /** Handlinger til høyre (knapper). */
    actions?: Snippet;
  }
  let { title, sub, description, back, actions }: Props = $props();
</script>

<div class="ph">
  {#if back}<a class="back" href={back.href}>{back.label}</a>{/if}
  <div class="row">
    <div class="txt">
      <h1 tabindex="-1">{title}{#if sub}{" "}<span class="sub">· {sub}</span>{/if}</h1>
      {#if description}<p class="desc">{description}</p>{/if}
    </div>
    {#if actions}<div class="act">{@render actions()}</div>{/if}
  </div>
</div>

<style>
  .ph { margin-block-end: var(--space-5); }
  .back { display: inline-block; margin-block-end: var(--space-1); color: var(--c-text-muted); font-size: var(--fs-sm); }
  .row { display: flex; flex-wrap: wrap; align-items: flex-end; justify-content: space-between; gap: var(--space-2) var(--space-4); }
  .txt { flex: 1 1 16rem; min-inline-size: 0; }
  h1 { margin: 0; font-size: var(--fs-xl); font-weight: var(--fw-semibold); line-height: var(--lh-tight); letter-spacing: -0.01em; }
  h1:focus { outline: none; }
  .sub { font-weight: var(--fw-regular); color: var(--c-text-muted); }
  .desc { margin: var(--space-1) 0 0; color: var(--c-text-muted); font-size: var(--fs-sm); max-inline-size: 62ch; }
  .act { display: flex; flex-wrap: wrap; gap: var(--space-2); }
</style>
