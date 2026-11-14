<!-- Appskall: ≥ 900 px venstre sidemeny (merke øverst, valg, fotfelt nederst), under 900 px toppmeny i samme DOM (samme fokusrekkefølge).
     Rotelementet er <header> (ett banner-landemerke); navigasjonen er <nav aria-label> med en liste. Ingen data hentes her. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import Logo from "./Logo.svelte";

  interface Props {
    /** Produktnavn ved logoen. */
    brand: string;
    /** Navn på navigasjonslandemerket (aria-label). */
    navLabel: string;
    /** NavItem-elementer (<li>). */
    children: Snippet;
    /** Nederst (sidemeny) / til høyre (toppmeny): f.eks. Hurtigtaster-knapp og versjon. */
    footer?: Snippet;
  }
  let { brand, navLabel, children, footer }: Props = $props();
</script>

<header class="side">
  <span class="brand"><span class="logo"><Logo size={24} /></span><span class="name">{brand}</span></span>
  <nav aria-label={navLabel}><ul>{@render children()}</ul></nav>
  {#if footer}<div class="foot">{@render footer()}</div>{/if}
</header>

<style>
  .side {
    display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-4);
    padding: var(--space-2) var(--gutter);
    background: var(--c-nav-bg); border-block-end: var(--border-width) solid var(--c-border-subtle);
  }
  /* Ordmerke (V2 2.8): symbol 24 px i aksent + «Fjell» i Inter 600, sporing -0.02em, 17 px. Ingen boks rundt. */
  .brand { display: inline-flex; align-items: center; gap: var(--space-2); font-size: 1.0625rem; font-weight: var(--fw-semibold); letter-spacing: var(--ls-tight); line-height: 1; }
  .logo { display: inline-flex; color: var(--c-accent); }
  nav { flex: 1 1 auto; min-inline-size: 0; }
  ul { display: flex; flex-wrap: wrap; gap: var(--space-1); list-style: none; margin: 0; padding: 0; }
  .foot { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-3); }
  .foot :global(.ver) { display: none; }
  @media (forced-colors: active) { .logo { color: CanvasText; } }

  @media (min-width: 56.25rem) {
    .side {
      position: sticky; inset-block-start: 0; block-size: 100vh; overflow-y: auto;
      flex-direction: column; flex-wrap: nowrap; align-items: stretch; gap: var(--space-5);
      padding: var(--space-5) var(--space-3) var(--space-4);
      border-inline-end: var(--border-width) solid var(--c-border-subtle);
      border-block-end: 0; border-inline-end: var(--border-width) solid var(--c-border-subtle);
    }
    .brand { padding-inline: var(--space-3); }
    nav { flex: 1 1 auto; }
    ul { flex-direction: column; flex-wrap: nowrap; gap: var(--space-1); }
    .foot { flex-direction: column; align-items: stretch; padding-inline: var(--space-1); }
    .foot :global(.ver) { display: block; }
  }
</style>
