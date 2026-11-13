<!-- Navigasjonsvalg i Sidebar. Lenke (ikke knapp): fungerer med hash-ruting. Teller står UTENFOR lenken (søsken), slik at lenkens
     tekst og tilgjengelige navn er kun etiketten; tellerens beskrivelse kobles med aria-describedby (f.eks. «24 bilder»). -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    href: string;
    label: string;
    /** Aktiv side: aria-current="page" + kantmarkering + vekt (aldri bare farge). */
    current?: boolean;
    /** Teller til høyre. Utelatt/null/0 = ingen teller. Allerede formatert tall (Intl). */
    count?: string | null;
    /** Skjermleserbeskrivelse av telleren, f.eks. «24 bilder». */
    countLabel?: string;
    icon?: Snippet;
  }
  let { href, label, current = false, count = null, countLabel, icon }: Props = $props();
  const uid = $props.id();
</script>

<li class="item" class:current>
  <a {href} aria-current={current ? "page" : undefined} aria-describedby={count && countLabel ? `${uid}-c` : undefined}>
    {#if icon}<span class="ico">{@render icon()}</span>{/if}
    <span class="lbl">{label}</span>
  </a>
  {#if count}
    <span class="cnt" aria-hidden="true">{count}</span>
    {#if countLabel}<span id="{uid}-c" class="sr-only">{countLabel}</span>{/if}
  {/if}
</li>

<style>
  .item { position: relative; display: flex; align-items: center; }
  a {
    display: flex; flex: 1; align-items: center; gap: var(--space-3);
    min-block-size: var(--target-min); padding: var(--space-2) var(--space-3);
    color: var(--c-text); text-decoration: none; font-weight: var(--fw-medium); font-size: var(--fs-md);
    border-inline-start: 3px solid transparent; border-radius: var(--radius-md);
    transition: background-color var(--dur-1) var(--ease-out), transform var(--dur-1) var(--ease-out);
  }
  a:hover { background: var(--c-surface-hover); }
  a:active { transform: scale(0.985); }
  .current a { background: var(--c-nav-active-bg); border-inline-start-color: var(--c-accent); font-weight: var(--fw-semibold); box-shadow: var(--shadow-1); }
  .ico { display: inline-flex; color: var(--c-text-muted); }
  .current .ico { color: var(--c-text); }
  .lbl { min-inline-size: 0; }
  .cnt {
    position: absolute; inset-inline-end: var(--space-3); pointer-events: none;
    min-inline-size: 1.5rem; padding: 0 var(--space-2); text-align: center;
    border-radius: var(--radius-pill); background: var(--c-surface-sunken); color: var(--c-text-muted);
    font-size: var(--fs-xs); font-weight: var(--fw-semibold); font-variant-numeric: var(--num); line-height: 1.4rem;
  }
  .current .cnt { background: var(--c-surface-sunken); color: var(--c-text); }
  @media (forced-colors: active) {
    .cnt { border: var(--border-width) solid CanvasText; background: Canvas; color: CanvasText; }
    .current a { border-inline-start-width: 5px; }
  }
  /* Smal: toppmeny. Ikon skjules, aktiv markeres med understrek, teller følger etiketten. */
  @media (max-width: 56.1875rem) {
    .item { flex: 0 0 auto; }
    a { border-inline-start: 0; border-block-end: 3px solid transparent; border-radius: var(--radius-sm); padding-inline: var(--space-3); }
    .current a { border-block-end-color: var(--c-text); box-shadow: none; background: transparent; }
    .ico { display: none; }
    .cnt { position: static; margin-inline-start: calc(var(--space-1) * -1); }
    .item { gap: 0; }
    a { flex: 0 1 auto; }
  }
</style>
