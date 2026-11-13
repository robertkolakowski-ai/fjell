<!-- Fremdrift / «X GB kan frigjøres». Ett element med full tekst for skjermleser (role="img" + label).
     Verdien står alltid som tekst i midten; ringen er kun supplement. -->
<script lang="ts">
  interface Props {
    /** 0..1 */
    fraction: number;
    /** Full tekst, f.eks. «12,4 GB kan frigjøres» (formatert i UI med Intl). */
    label: string;
    /** Stor verdi i midten, f.eks. «12,4 GB». Utelatt: label vises. */
    value?: string;
    /** Liten tekst under verdien, f.eks. «kan frigjøres». */
    caption?: string;
    /** sm = 8 rem, md (standard, kompakt) = 11 rem, lg = 15 rem. */
    size?: "sm" | "md" | "lg";
  }
  let { fraction, label, value, caption, size = "md" }: Props = $props();

  const R = 52;
  const C = 2 * Math.PI * R;
  const f = $derived(Math.min(1, Math.max(0, Number.isFinite(fraction) ? fraction : 0)));
</script>

<div class="ring {size}" role="img" aria-label={label}>
  <svg viewBox="0 0 120 120" aria-hidden="true" focusable="false">
    <circle class="track" cx="60" cy="60" r={R} />
    <circle class="fill" cx="60" cy="60" r={R} stroke-dasharray={C} stroke-dashoffset={C * (1 - f)} transform="rotate(-90 60 60)" />
  </svg>
  <div class="cap" aria-hidden="true">
    <span class="value">{value ?? label}</span>
    {#if caption}<span class="caption">{caption}</span>{/if}
  </div>
</div>

<style>
  .ring {
    position: relative;
    margin: 0;
    inline-size: min(100%, 11rem);
    aspect-ratio: 1;
    flex: none;
  }
  .ring.sm { inline-size: min(100%, 8rem); }
  .ring.lg { inline-size: min(100%, 15rem); }
  svg { inline-size: 100%; block-size: 100%; }
  circle { fill: none; stroke-width: 9; }
  .track { stroke: var(--c-ring-track); opacity: 0.45; }
  .fill { stroke: var(--c-ring-fill); stroke-linecap: round; transition: stroke-dashoffset var(--dur-base) ease-out; }
  .cap {
    position: absolute;
    inset: 18%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    min-inline-size: 0;
  }
  .value { font-size: var(--fs-lg); font-weight: var(--fw-semibold); line-height: var(--lh-tight); font-variant-numeric: var(--num); overflow-wrap: normal; }
  .lg .value { font-size: var(--fs-xl); }
  .caption { font-size: var(--fs-xs); color: var(--c-text-muted); }
  @media (forced-colors: active) {
    .track { stroke: GrayText; opacity: 1; }
    .fill { stroke: Highlight; }
  }
</style>
