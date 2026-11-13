<!-- Knapp. primary = eneste aksentfarge på skjermen (én primærhandling per skjerm). Vokser i høyde ved lang tekst. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    variant?: "primary" | "secondary" | "quiet" | "danger" | "ok";
    /** sm = tett knapp for rader og verktøylinjer (≥ 32 px). */
    size?: "md" | "sm";
    type?: "button" | "submit";
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    /** Synlig tekst er også tilgjengelig navn (WCAG 2.5.3). Sett kun aria-label hvis den inneholder den synlige teksten. */
    ariaLabel?: string;
    ariaKeyshortcuts?: string;
    /** Synlig sperret, men fortsatt fokuserbar og med forklaring (`ariaDescribedby`): ingen skjult sperre. Klikk gjør ingenting. */
    ariaDisabled?: boolean;
    /** id til elementet som forklarer hvorfor knappen er sperret. */
    ariaDescribedby?: string;
    /** Knapp på mørk scene (Kortmodus, zoom): lys tekst på gjennomsiktig flate. */
    onStage?: boolean;
    children: Snippet;
  }
  let { variant = "secondary", size = "md", type = "button", disabled = false, onclick, ariaLabel, ariaKeyshortcuts, ariaDisabled = false, ariaDescribedby, onStage = false, children }: Props = $props();
</script>

<button class="btn {variant} {size}" class:stage={onStage} {type} {disabled} onclick={(e) => { if (!ariaDisabled) onclick?.(e); }} aria-label={ariaLabel}
  aria-keyshortcuts={ariaKeyshortcuts} aria-disabled={ariaDisabled ? "true" : undefined} aria-describedby={ariaDescribedby}>
  {@render children()}
</button>

<style>
  .btn {
    min-block-size: var(--target-min);
    min-inline-size: var(--target-min);
    padding: var(--space-2) var(--space-4);
    border: var(--border-width) solid var(--c-border-strong);
    border-radius: var(--radius-sm);
    font: inherit;
    font-size: var(--fs-md);
    font-weight: var(--fw-medium);
    line-height: var(--lh-tight);
    text-align: center;
    cursor: pointer;
    background: var(--c-surface-raised);
    color: var(--c-text);
    transition: background-color var(--dur-1) var(--ease-out), border-color var(--dur-1) var(--ease-out), transform var(--dur-1) var(--ease-out);
  }
  .btn:active:not(:disabled):not([aria-disabled="true"]) { transform: scale(0.98); }
  .btn.sm { min-block-size: var(--control-sm); min-inline-size: var(--control-sm); padding: var(--space-1) var(--space-3); font-size: var(--fs-sm); }
  /* Rolig knapp: ingen synlig kant før hover (kanten finnes, men er gjennomsiktig, så den overlever høykontrast). */
  .btn.quiet { background: transparent; border-color: transparent; }
  .btn.quiet:hover:not(:disabled):not([aria-disabled="true"]) { background: var(--c-surface-hover); border-color: var(--c-border-strong); }
  .btn.secondary:hover:not(:disabled):not([aria-disabled="true"]) { background: var(--c-surface-hover); }
  /* Semantiske konturknapper: rust = flytter til papirkurv/tømmer, mose = beholder. Aldri aksent (V2 2.1). */
  .btn.danger { color: var(--c-danger); border-color: var(--c-danger); }
  .btn.danger:hover:not(:disabled):not([aria-disabled="true"]) { background: var(--c-danger-bg); }
  .btn.ok { color: var(--c-ok); border-color: var(--c-ok); }
  .btn.ok:hover:not(:disabled):not([aria-disabled="true"]) { background: var(--c-ok-bg); }
  .btn.primary {
    background: var(--c-accent);
    color: var(--c-on-accent);
    border-color: var(--c-accent);
    font-weight: var(--fw-semibold);
  }
  .btn.primary:hover:not(:disabled):not([aria-disabled="true"]) { background: var(--c-accent-hover); border-color: var(--c-accent-hover); }
  /* På scenen (Kortmodus, zoom): gjennomsiktig flate, lys tekst, scenens egne semantikkfarger. */
  .btn.stage { background: transparent; color: var(--c-stage-text); border-color: var(--c-stage-muted); }
  .btn.stage:hover:not(:disabled):not([aria-disabled="true"]) { background: var(--c-stage-border); }
  .btn.stage.danger { color: var(--c-stage-danger); border-color: var(--c-stage-danger); }
  .btn.stage.ok { color: var(--c-stage-ok); border-color: var(--c-stage-ok); }
  .btn.stage:focus-visible { outline-color: var(--c-stage-focus); }
  .btn:disabled, .btn[aria-disabled="true"] {
    cursor: not-allowed;
    opacity: 0.6; /* tekst + tilstand er også kodet via disabled-attributtet; deaktivert er unntatt fra kontrastkrav */
    border-style: dashed;
  }
  @media (forced-colors: active) {
    .btn, .btn.quiet, .btn.danger, .btn.ok, .btn.stage { border-color: ButtonText; }
    .btn.primary { background: ButtonFace; border-width: 3px; }
    .btn:disabled, .btn[aria-disabled="true"] { color: GrayText; border-color: GrayText; opacity: 1; }
  }
</style>
