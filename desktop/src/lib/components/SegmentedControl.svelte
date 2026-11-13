<!-- Segmentvelger: radiogruppe (native radioknapper, så piltaster, roving fokus og Tab-oppførsel er nettleserens egne).
     Valgt segment har fylt flate, hake og vekt (ikke bare farge). Bryter til flere rader ved lang tekst. -->
<script lang="ts">
  import Icon from "./Icon.svelte";

  interface Option { value: string; label: string }
  interface Props {
    options: Option[];
    value: string;
    onchange: (value: string) => void;
    /** Gruppens navn for skjermleser (aria-label), hvis ikke labelledby er satt. */
    ariaLabel?: string;
    ariaLabelledby?: string;
    ariaDescribedby?: string;
    disabled?: boolean;
    /** Felles name for radioknappene (default: unik). */
    name?: string;
  }
  let { options, value, onchange, ariaLabel, ariaLabelledby, ariaDescribedby, disabled = false, name }: Props = $props();
  const uid = $props.id();
</script>

<div class="seg" role="radiogroup" aria-label={ariaLabelledby ? undefined : ariaLabel} aria-labelledby={ariaLabelledby} aria-describedby={ariaDescribedby}>
  {#each options as o (o.value)}
    <label class:on={value === o.value} class:disabled>
      <input type="radio" name={name ?? uid} value={o.value} checked={value === o.value} {disabled} onchange={() => onchange(o.value)} />
      <span class="face">{#if value === o.value}<Icon name="check" />{/if}<span>{o.label}</span></span>
    </label>
  {/each}
</div>

<style>
  .seg { display: inline-flex; flex-wrap: wrap; gap: var(--space-1); padding: var(--space-1); max-inline-size: 100%; background: var(--c-surface-sunken); border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-md); }
  label { position: relative; display: flex; }
  input { position: absolute; inset: 0; inline-size: 100%; block-size: 100%; margin: 0; opacity: 0; cursor: pointer; }
  .face {
    display: inline-flex; align-items: center; justify-content: center; gap: var(--space-1);
    min-block-size: calc(var(--target-min) - var(--space-2)); padding: var(--space-1) var(--space-3);
    border: var(--border-width) solid transparent; border-radius: var(--radius-sm);
    font-size: var(--fs-sm); font-weight: var(--fw-medium); text-align: center; color: var(--c-text);
  }
  .face :global(.icon) { inline-size: 1em; block-size: 1em; }
  label:hover:not(.on) .face { background: var(--c-surface-hover); }
  .on .face { background: var(--c-surface); border-color: var(--c-border-strong); font-weight: var(--fw-semibold); box-shadow: var(--shadow-card); }
  input:focus-visible + .face { outline: var(--focus-width) solid var(--c-focus); outline-offset: var(--focus-offset); }
  .disabled .face { opacity: 0.6; }
  .disabled input { cursor: not-allowed; }
  @media (forced-colors: active) {
    .on .face { border-width: 3px; }
    .disabled .face { color: GrayText; opacity: 1; }
  }
</style>
