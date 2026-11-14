<!-- Kort: flate med valgfri overskrift. Innholdet styrer layouten; ingen faste bredder på tekst. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    heading?: string;
    headingLevel?: 2 | 3 | 4;
    /** «raised» (standard, flate nivå 1 med tynn kant og lav skygge) eller «sunken» (innfelt nivå 2, uten skygge). */
    tone?: "raised" | "sunken";
    /** Innvendig luft. «none» for kort som har egne rader/bilder helt ut til kanten. */
    padding?: "md" | "none";
    children: Snippet;
  }
  let { heading, headingLevel = 2, tone = "raised", padding = "md", children }: Props = $props();
  const uid = $props.id();
  const tag = $derived(`h${headingLevel}`);
</script>

<!-- Gruppe, ikke region: et landemerke per kort gir støy i Narrator/NVDA-landemerkelisten, og aria-label som gjentar
     overskriften leses to ganger. Navnet kommer fra selve overskriften. -->
<div class="card {tone} pad-{padding}" role={heading ? "group" : undefined} aria-labelledby={heading ? `${uid}-t` : undefined}>
  {#if heading}
    <svelte:element this={tag} id="{uid}-t" class="title">{heading}</svelte:element>
  {/if}
  {@render children()}
</div>

<style>
  .card {
    background: var(--c-surface);
    color: var(--c-text);
    border: var(--border-width) solid var(--c-border-subtle);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-1);
    padding: var(--space-4) var(--space-5);
    min-inline-size: 0;
    animation: card-in var(--dur-3) var(--ease-out) backwards;
  }
  .sunken { background: var(--c-surface-sunken); box-shadow: none; }
  .pad-none { padding: 0; }
  .title {
    margin: 0 0 var(--space-3);
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-heading);
  }
  /* Kort inn: kort toning og 4 px løft (bare opacity/transform). Av ved redusert bevegelse (base.css). */
  @keyframes card-in { from { opacity: 0; transform: translateY(0.25rem); } to { opacity: 1; transform: none; } }
</style>
