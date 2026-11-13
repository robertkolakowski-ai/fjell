<!-- Tallflis: etikett, stort tall (tabulært), støtteverdi. Med href blir hele flisen en lenke (kategorikort på Hjem). -->
<script lang="ts">
  import Icon from "./Icon.svelte";

  interface Props {
    label: string;
    /** Hovedverdi, ferdig formatert (Intl), f.eks. «5,7 MB». */
    value: string;
    /** Liten tekst under, f.eks. «6 bilder». */
    hint?: string;
    /** Gjør flisen til en lenke. */
    href?: string;
    tone?: "default" | "ok" | "warn" | "danger";
  }
  let { label, value, hint, href, tone = "default" }: Props = $props();
</script>

{#snippet body()}
  <span class="label">{label}</span>
  <span class="value">{value}</span>
  {#if hint}<span class="hint">{hint}</span>{/if}
  {#if href}<span class="chev" aria-hidden="true"><Icon name="chevron" /></span>{/if}
{/snippet}

{#if href}
  <a class="stat {tone}" {href}>{@render body()}</a>
{:else}
  <div class="stat {tone}">{@render body()}</div>
{/if}

<style>
  .stat {
    position: relative; display: flex; flex-direction: column; gap: var(--space-1);
    padding: var(--space-4) var(--space-5) var(--space-4) var(--space-4); min-inline-size: 0;
    color: var(--c-text); text-decoration: none;
    background: var(--c-surface); border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-lg); box-shadow: var(--shadow-card);
  }
  a.stat:hover { background: var(--c-surface-hover); border-color: var(--c-border-strong); }
  .label { font-weight: var(--fw-semibold); }
  .value { margin-block-start: var(--space-1); font-size: var(--fs-stat); font-weight: var(--fw-semibold); line-height: var(--lh-tight); font-variant-numeric: var(--num); }
  .ok .value { color: var(--c-ok); }
  .danger .value { color: var(--c-danger); }
  .hint { color: var(--c-text-muted); font-size: var(--fs-sm); font-variant-numeric: var(--num); }
  .chev { position: absolute; inset-inline-end: var(--space-3); inset-block-start: var(--space-4); color: var(--c-text-muted); }
  .warn { border-inline-start: 4px solid var(--c-border-strong); }
  @media (forced-colors: active) { a.stat:hover { border-color: Highlight; } }
</style>
