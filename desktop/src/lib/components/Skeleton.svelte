<!-- Skjelett mens innhold lastes. Dekorativ (aria-hidden): sett aria-busy på beholderen og la en LiveRegion si fra når data er der.
     «thumb» er helt rolig (V2: ingen shimmer på miniatyrer), «text» og «block» pulserer svakt (bare opacity; av ved redusert bevegelse).
     Høykontrast: tynn kontur i stedet for flate. -->
<script lang="ts">
  interface Props {
    variant?: "text" | "block" | "thumb";
    /** Antall tekstlinjer (variant «text»). Siste linje er kortere. */
    lines?: number;
    /** Bredde/høyde som CSS-verdier (rem eller %). Standard fyller bredden; «block» og «thumb» bruker `ratio`. */
    width?: string;
    height?: string;
    /** Sideforhold for «thumb» og «block» uten høyde, f.eks. «16 / 10». */
    ratio?: string;
  }
  let { variant = "block", lines = 3, width, height, ratio = "16 / 10" }: Props = $props();
  const n = $derived(Math.max(1, Math.min(8, Math.floor(lines))));
</script>

{#if variant === "text"}
  <span class="sk lines" aria-hidden="true" style:inline-size={width}>
    {#each Array.from({ length: n }, (_, i) => i) as i (i)}<span class="ln" class:last={i === n - 1 && n > 1}></span>{/each}
  </span>
{:else}
  <span class="sk {variant}" aria-hidden="true" style:inline-size={width} style:block-size={height} style:aspect-ratio={height ? undefined : ratio}></span>
{/if}

<style>
  .sk { display: block; max-inline-size: 100%; }
  .block, .thumb, .ln { background: var(--c-surface-sunken); border-radius: var(--radius-sm); }
  .thumb { background: var(--c-well); border-radius: var(--radius-xs); }
  .block { border-radius: var(--radius-md); }
  .lines { display: grid; gap: var(--space-2); }
  .ln { block-size: 0.75rem; }
  .ln.last { inline-size: 62%; }
  @media (prefers-reduced-motion: no-preference) {
    .block, .ln { animation: pulse 1.6s var(--ease-move) infinite alternate; }
  }
  @keyframes pulse { from { opacity: 1; } to { opacity: 0.55; } }
  @media (forced-colors: active) {
    .block, .thumb, .ln { background: Canvas; border: 1px solid GrayText; }
  }
</style>
