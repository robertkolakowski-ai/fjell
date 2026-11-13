<!-- Bildefliste i rutenett. En knapp med aria-pressed (valgt / ikke valgt). Tilgjengelig navn er metadata
     (a11y.photo.*), ikke bildebeskrivelse (ingen ML i MVP). Valgt = hake + tykk kant, ikke bare farge.
     Maks 2 merker. «Beste» er eget merke med tekst. Ingen kobling til bridge: src, tekster og tilstand kommer inn som props. -->
<script lang="ts">
  import Badge from "./Badge.svelte";
  import { tr } from "../i18n";
  import Thumb from "./Thumb.svelte";

  interface Props {
    /** Full tilgjengelig etikett, f.eks. «Bilde 3 av 12. Tatt 14. juli 2025. 3,2 MB. Uskarpt 82 %.» */
    label: string;
    src?: string;
    /** Bilde-id (`asset`): viser miniatyr via bridge (lazy, med skeleton og forklart feil). Har `src` forrang. */
    thumbId?: number;
    selected?: boolean;
    /** Hindrer valg (f.eks. keeper, beskyttet bilde). Tilstanden er synlig som tekst via lockedLabel. */
    disabled?: boolean;
    onToggle?: (selected: boolean) => void;
    /** Begrunnelsesmerker (maks 2 vises). */
    badges?: { label: string; tone?: "neutral" | "warn" }[];
    /** Tekst for «Beste»-markering, f.eks. «Beste». */
    bestLabel?: string;
  }
  let { label, src, thumbId, selected = false, disabled = false, onToggle, badges = [], bestLabel }: Props = $props();
  let thumbErr: string | null = $state(null);
  const KNOWN = ["unavailable", "cloud_placeholder", "corrupt", "not_found", "no_library"];
  /** Forklaringen tegnes som generert innhold i flisen (ikke tekstnode, ellers krever WCAG 2.5.3 at hele den synlige teksten står i navnet i riktig rekkefølge); navnet bærer den for hjelpemidler. */
  const explainErr = (code: string): string => $tr(`thumb.error.${KNOWN.includes(code) ? code : "unavailable"}` as "thumb.error.unavailable");
</script>

<button class="tile" class:selected type="button" aria-pressed={selected} aria-label={thumbErr ? `${label} ${explainErr(thumbErr)}` : label} {disabled} onclick={() => onToggle?.(!selected)}>
  <span class="img">
    {#if src}<img {src} alt="" loading="lazy" decoding="async" />{:else if thumbId !== undefined}<Thumb id={thumbId} generated onerror={(c) => (thumbErr = c)} />{:else}<span class="ph" aria-hidden="true"></span>{/if}
    {#if selected}
      <svg class="check" viewBox="0 0 24 24" aria-hidden="true" focusable="false"><circle cx="12" cy="12" r="11" /><path d="M6.5 12.5l3.8 3.8 7.2-7.6" /></svg>
    {/if}
  </span>
  {#if bestLabel || badges.length}
    <span class="meta" aria-hidden="true">
      {#if bestLabel}<Badge label={bestLabel} />{/if}
      {#each badges.slice(0, 2) as b (b.label)}<Badge label={b.label} tone={b.tone} />{/each}
    </span>
  {/if}
</button>

<style>
  .tile {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    inline-size: 100%;
    min-inline-size: 0;
    padding: var(--space-2);
    border: var(--border-width) solid var(--c-border);
    border-radius: var(--radius-md);
    background: var(--c-surface);
    color: var(--c-text);
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .tile.selected {
    border: 3px solid var(--c-selected);
    padding: calc(var(--space-2) - 2px);
  }
  .tile:disabled { cursor: not-allowed; }
  .img {
    position: relative;
    display: block;
    aspect-ratio: 4 / 3;
    border-radius: var(--radius-sm);
    overflow: hidden;
    background: var(--c-surface-sunken);
  }
  img, .ph { inline-size: 100%; block-size: 100%; object-fit: cover; display: block; }
  .ph { background: repeating-linear-gradient(45deg, var(--c-surface-sunken), var(--c-surface-sunken) 8px, var(--c-border) 8px, var(--c-border) 9px); }
  .check { position: absolute; inset-block-start: var(--space-2); inset-inline-end: var(--space-2); inline-size: 1.75rem; block-size: 1.75rem; }
  .check circle { fill: var(--c-selected); stroke: var(--c-surface); stroke-width: 1.5; }
  .check path { fill: none; stroke: var(--c-surface); stroke-width: 2.6; stroke-linecap: round; stroke-linejoin: round; }
  .meta { display: flex; flex-wrap: wrap; gap: var(--space-1); min-inline-size: 0; }
  @media (forced-colors: active) {
    .tile.selected { border-color: Highlight; }
    .check circle { fill: Highlight; stroke: Canvas; }
    .check path { stroke: HighlightText; }
  }
</style>
