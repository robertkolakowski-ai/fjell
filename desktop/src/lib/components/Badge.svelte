<!-- Begrunnelsesmerke, f.eks. «Uskarpt (82 %)». Alltid en hel streng fra i18n (aldri sammensatt i kode).
     Ikke bare farge: ikon + tekst + kant. Kan avkortes med ellipsis, men full tekst ligger i DOM (skjermleser) og title. -->
<script lang="ts">
  interface Props {
    label: string;
    tone?: "neutral" | "warn";
  }
  let { label, tone = "neutral" }: Props = $props();
</script>

<span class="badge {tone}" title={label}>
  <svg class="ico" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
    {#if tone === "warn"}
      <path d="M8 1.5 15 14H1z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" /><path d="M8 6v4M8 11.6v.9" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
    {:else}
      <circle cx="8" cy="8" r="6" fill="none" stroke="currentColor" stroke-width="1.6" /><path d="M8 7.2v4M8 4.6v.9" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
    {/if}
  </svg>
  <span class="txt">{label}</span>
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    max-inline-size: 100%;
    padding: 0.125rem var(--space-2);
    border: var(--border-width) solid var(--c-border-strong);
    border-radius: var(--radius-pill);
    background: var(--c-badge-bg);
    color: var(--c-badge-text);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
  }
  .badge.warn { background: var(--c-badge-warn-bg); color: var(--c-badge-warn-text); }
  .ico { inline-size: 1em; block-size: 1em; flex: none; }
  .txt { min-inline-size: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
