<!-- Beskyttelsesbryter: slår beskyttelse av bilde, mappe, album eller person av og på. Tilgjengelig av/på med ikon, tekst og
     aria-pressed (tilstand bæres aldri av farge alene). Navnet er `label` og endres ikke når tilstanden endres; tilstandsordet
     «På/Av» er bare visuelt (aria-pressed sier det samme til skjermleser). Beskyttede bilder krever alltid egen bekreftelse
     før de flyttes til papirkurven; det forklares i en skjult beskrivelse. Ingen bridge-avhengighet: tilstanden eies av kalleren,
     som kaller `protect(scope, next)` og oppdaterer `protected` (kan angres med motsatt kall). -->
<script lang="ts">
  import type { Scope } from "../../bridge/types-v1";
  import { tr } from "../i18n";
  import Icon from "./Icon.svelte";

  interface Props {
    scope: Scope;
    protected: boolean;
    /** Kalles med ønsket nye tilstand. Bryteren bytter ikke selv; kalleren oppdaterer `protected` når kjernen har svart. */
    onchange: (next: boolean) => void;
    /** Tilgjengelig navn, f.eks. «Beskytt dette bildet». Synlig tekst. */
    label: string;
    disabled?: boolean;
  }
  let { scope, protected: on, onchange, label, disabled = false }: Props = $props();
  const uid = $props.id();
  const scopeKey = $derived(`protect.scope.${scope.scope}` as const);
</script>

<button type="button" class="toggle" class:on {disabled} aria-pressed={on} aria-describedby="{uid}-d" data-scope={scope.scope}
  onclick={() => onchange(!on)}>
  <Icon name={on ? "check" : "shield"} filled={on} />
  <span class="text">{label}</span>
  <span class="state" aria-hidden="true">{$tr(on ? "protect.toggle.on" : "protect.toggle.off")}</span>
</button>
<span id="{uid}-d" class="sr-only">{$tr("protect.toggle.hint", { scope: $tr(scopeKey) })}</span>

<style>
  .toggle {
    display: inline-flex; align-items: center; gap: var(--space-2);
    min-block-size: var(--target-min); min-inline-size: var(--target-min);
    padding: var(--space-2) var(--space-3);
    border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-sm);
    background: var(--c-surface-raised); color: var(--c-text);
    font: inherit; font-size: var(--fs-md); font-weight: var(--fw-medium); line-height: var(--lh-tight); text-align: start; cursor: pointer;
    transition: background-color var(--dur-1) var(--ease-out), border-color var(--dur-1) var(--ease-out);
  }
  .toggle:hover:not(:disabled) { background: var(--c-surface-hover); }
  .toggle:disabled { cursor: default; opacity: 0.6; }
  /* På: tykkere kant, fylt flate og hake (ikke bare farge; kantbredden overlever høykontrast). */
  .toggle.on { border-width: 3px; background: var(--c-ok-bg); border-color: var(--c-ok); }
  .text { min-inline-size: 0; overflow-wrap: anywhere; }
  .state {
    margin-inline-start: auto; padding: 0 var(--space-2);
    border: var(--border-width) solid currentColor; border-radius: var(--radius-pill);
    font-size: var(--fs-xs); font-weight: var(--fw-semibold); white-space: nowrap;
  }
  @media (forced-colors: active) {
    .toggle.on { border-color: Highlight; }
  }
</style>
