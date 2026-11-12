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
  }
  let { fraction, label, value, caption }: Props = $props();

  const R = 52;
  const C = 2 * Math.PI * R;
  const f = $derived(Math.min(1, Math.max(0, Number.isFinite(fraction) ? fraction : 0)));
</script>

<figure class="ring" role="img" aria-label={label}>
  <svg viewBox="0 0 120 120" aria-hidden="true" focusable="false">
    <circle class="track" cx="60" cy="60" r={R} />
    <circle class="fill" cx="60" cy="60" r={R} stroke-dasharray={C} stroke-dashoffset={C * (1 - f)} transform="rotate(-90 60 60)" />
  </svg>
  <figcaption aria-hidden="true">
    <span class="value">{value ?? label}</span>
    {#if caption}<span class="caption">{caption}</span>{/if}
  </figcaption>
</figure>

<style>
  .ring {
    position: relative;
    margin: 0;
    inline-size: min(100%, 15rem);
    aspect-ratio: 1;
  }
  svg { inline-size: 100%; block-size: 100%; }
  circle { fill: none; stroke-width: 10; }
  .track { stroke: var(--c-ring-track); }
  .fill { stroke: var(--c-ring-fill); stroke-linecap: round; transition: stroke-dashoffset var(--dur-base) ease-out; }
  figcaption {
    position: absolute;
    inset: 18%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    min-inline-size: 0;
  }
  .value { font-size: var(--fs-xl); font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .caption { font-size: var(--fs-sm); color: var(--c-text-muted); }
  @media (forced-colors: active) {
    .track { stroke: GrayText; }
    .fill { stroke: Highlight; }
  }
</style>
