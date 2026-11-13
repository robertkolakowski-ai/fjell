<!-- Knapp. primary = eneste aksentfarge på skjermen (én primærhandling per skjerm). Vokser i høyde ved lang tekst. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    variant?: "primary" | "secondary" | "quiet" | "danger";
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
    children: Snippet;
  }
  let { variant = "secondary", size = "md", type = "button", disabled = false, onclick, ariaLabel, ariaKeyshortcuts, ariaDisabled = false, ariaDescribedby, children }: Props = $props();
</script>

<button class="btn {variant} {size}" {type} {disabled} onclick={(e) => { if (!ariaDisabled) onclick?.(e); }} aria-label={ariaLabel}
  aria-keyshortcuts={ariaKeyshortcuts} aria-disabled={ariaDisabled ? "true" : undefined} aria-describedby={ariaDescribedby}>
  {@render children()}
</button>

<style>
  .btn {
    min-block-size: var(--target-min);
    min-inline-size: var(--target-min);
    padding: var(--space-2) var(--space-4);
    border: var(--border-width) solid var(--c-border-strong);
    font-size: var(--fs-md);
    border-radius: var(--radius-md);
    font: inherit;
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
    text-align: center;
    cursor: pointer;
    background: var(--c-surface);
    color: var(--c-text);
    transition: background-color var(--dur-fast);
  }
  .btn.sm { min-block-size: var(--control-sm); min-inline-size: var(--control-sm); padding: var(--space-1) var(--space-3); font-size: var(--fs-sm); }
  /* Rolig knapp: ingen synlig kant før hover (kanten finnes, men er gjennomsiktig, så den overlever høykontrast). */
  .btn.quiet { background: transparent; border-color: transparent; }
  .btn.quiet:hover:not(:disabled):not([aria-disabled="true"]) { background: var(--c-surface-hover); border-color: var(--c-border-strong); }
  .btn.danger { color: var(--c-danger); border-color: var(--c-danger); }
  .btn.danger:hover:not(:disabled):not([aria-disabled="true"]) { background: var(--c-danger-bg); }
  .btn.primary {
    background: var(--c-accent);
    color: var(--c-on-accent);
    border-color: var(--c-accent);
  }
  .btn.primary:hover:not(:disabled):not([aria-disabled="true"]) { background: var(--c-accent-hover); border-color: var(--c-accent-hover); }
  .btn.secondary:hover:not(:disabled):not([aria-disabled="true"]) { background: var(--c-surface-hover); }
  .btn:disabled, .btn[aria-disabled="true"] {
    cursor: not-allowed;
    opacity: 0.6; /* tekst + tilstand er også kodet via disabled-attributtet; deaktivert er unntatt fra kontrastkrav */
    border-style: dashed;
  }
  @media (forced-colors: active) {
    .btn, .btn.quiet, .btn.danger { border-color: ButtonText; }
    .btn.primary { background: ButtonFace; border-width: 3px; }
    .btn:disabled, .btn[aria-disabled="true"] { color: GrayText; border-color: GrayText; opacity: 1; }
  }
</style>
