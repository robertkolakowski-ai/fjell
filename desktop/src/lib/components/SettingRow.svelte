<!-- Én innstilling: tittel + forklaring til venstre, kontroll til høyre (stables i smale kort). Hører hjemme i SettingsCard.
     `control` får ids slik at kontrollen kan peke på tittel og forklaring (aria-labelledby/-describedby).
     Med `labelFor` er tittelen en <label for>; ellers vanlig tekst. Ingen kontroll = ren informasjonsrad. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    label: string;
    description?: string;
    /** id til et enkelt skjemafelt (select, input) tittelen skal merke. */
    labelFor?: string;
    control?: Snippet<[{ labelId: string; descId: string }]>;
    /** Ekstra innhold under raden (status, sti). */
    children?: Snippet;
  }
  let { label, description, labelFor, control, children }: Props = $props();
  const uid = $props.id();
  const ids = { labelId: `${uid}-l`, descId: `${uid}-d` };
</script>

<div class="sr">
  <div class="txt">
    {#if labelFor}<label id={ids.labelId} for={labelFor}>{label}</label>{:else}<span id={ids.labelId} class="lbl">{label}</span>{/if}
    {#if description}<p id={ids.descId}>{description}</p>{/if}
  </div>
  {#if control}<div class="ctl">{@render control(ids)}</div>{/if}
  {#if children}<div class="more">{@render children()}</div>{/if}
</div>

<style>
  .sr { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: var(--space-2) var(--space-5); padding: var(--space-3) var(--space-5); border-block-start: var(--border-width) solid var(--c-border-subtle); }
  .sr:first-child { border-block-start: 0; }
  .lbl, label { font-weight: var(--fw-medium); }
  label { display: block; }
  p { margin: var(--space-0) 0 0; color: var(--c-text-muted); font-size: var(--fs-sm); overflow-wrap: anywhere; }
  .ctl { justify-self: end; min-inline-size: 0; max-inline-size: 100%; }
  .more { grid-column: 1 / -1; }
  @container (max-width: 34rem) {
    .sr { grid-template-columns: minmax(0, 1fr); }
    .ctl { justify-self: start; }
  }
</style>
