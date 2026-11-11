<!-- Fast bunnlinje: primærhandling med sum, f.eks. «Flytt 87 til papirkurv (1,2 GB)».
     Hele teksten kommer ferdig formatert fra i18n. Teksten bryter til flere linjer og knappen vokser (norsk +30 %).
     Ny sum annonseres via LiveRegion med 300 ms demping. Egen avstandsholder hindrer at innhold skjules. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import Button from "./Button.svelte";
  import LiveRegion from "./LiveRegion.svelte";

  interface Props {
    /** Handlingsteksten, også tilgjengelig navn. */
    label: string;
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    /** Valgfri tekst som annonseres når summen endres (default: label). */
    announce?: string;
    /** Sekundære handlinger (f.eks. «Angre», «Velg alle»), vises før primærknappen. */
    secondary?: Snippet;
    ariaLabel?: string;
  }
  let { label, disabled = false, onclick, announce, secondary, ariaLabel }: Props = $props();
</script>

<div class="spacer" aria-hidden="true"></div>
<div class="bar" role="region" aria-label={ariaLabel}>
  <div class="inner">
    {#if secondary}<div class="sec">{@render secondary()}</div>{/if}
    <div class="primary">
      <Button variant="primary" {disabled} {onclick}>{label}</Button>
    </div>
  </div>
  <LiveRegion message={disabled ? "" : (announce ?? label)} delay={300} />
</div>

<style>
  .spacer { block-size: calc(var(--target-min) + var(--space-5) * 2 + var(--space-5)); }
  .bar {
    position: fixed;
    inset-inline: 0;
    inset-block-end: 0;
    z-index: 10;
    padding: var(--space-3) var(--space-4) calc(var(--space-3) + env(safe-area-inset-bottom, 0px));
    background: var(--c-surface);
    border-block-start: var(--border-width) solid var(--c-border);
    box-shadow: var(--shadow-lg);
  }
  .inner {
    display: flex;
    flex-wrap: wrap;
    align-items: stretch;
    justify-content: flex-end;
    gap: var(--space-3);
    max-inline-size: 60rem;
    margin-inline: auto;
  }
  .sec { display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .primary { flex: 1 1 14rem; display: flex; }
  .primary :global(.btn) { flex: 1; }
</style>
